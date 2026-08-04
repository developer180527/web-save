import BackgroundTasks
import Flutter
import UIKit

/// The Rust entry point, linked straight into this binary.
///
/// Cargokit compiles the bridge crate as a static library and force-loads it,
/// so this symbol is already in the executable — no Flutter engine, no Dart VM
/// and no method channel are involved in calling it. That matters here
/// specifically: a BGTask handler runs with no engine, and standing one up
/// would spend most of the budget iOS grants before checking a single link.
///
/// Returns the number of links checked, or a negative code (see background.rs).
@_silgen_name("websave_run_due_checks")
func websave_run_due_checks(_ vaultPath: UnsafePointer<CChar>?) -> Int32

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  /// Must match BGTaskSchedulerPermittedIdentifiers in Info.plist exactly.
  /// A mismatch is not an error at registration — the task simply never runs.
  private static let refreshTaskID = "com.venugopal.websaveApp.linkcheck"

  /// Roughly how long to wait before the next sweep is *eligible*.
  ///
  /// A floor, not a schedule. iOS decides when these actually run from how the
  /// app is used, whether it is on power, and how much budget it has left — so
  /// this may fire in an hour or not until tomorrow, and never at all on a
  /// device low on battery.
  private static let refreshInterval: TimeInterval = 2 * 60 * 60

  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    // Registration has to happen before launching finishes; iOS throws if a
    // handler is registered late, so this stays above the super call.
    BGTaskScheduler.shared.register(
      forTaskWithIdentifier: Self.refreshTaskID,
      using: nil
    ) { task in
      self.handleLinkCheck(task)
    }

    scheduleLinkCheck()
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
  }

  override func applicationDidEnterBackground(_ application: UIApplication) {
    // Requests are cleared once they run, and one made while the app is in
    // use is the most likely to be honoured, so ask again on the way out as
    // well as at launch.
    scheduleLinkCheck()
    super.applicationDidEnterBackground(application)
  }

  // MARK: - Scheduling

  private func scheduleLinkCheck() {
    let request = BGAppRefreshTaskRequest(identifier: Self.refreshTaskID)
    request.earliestBeginDate = Date(timeIntervalSinceNow: Self.refreshInterval)
    do {
      try BGTaskScheduler.shared.submit(request)
    } catch {
      // Expected and harmless in the simulator, which has no scheduler, and
      // when a request is already pending. Nothing the user can act on.
      NSLog("WebSave: could not schedule link check: \(error)")
    }
  }

  // MARK: - Running

  private func handleLinkCheck(_ task: BGTask) {
    // Schedule the replacement first. iOS does not repeat a task for you, and
    // everything after this point can be cut short — losing the next run
    // because this one was killed would end background checking permanently
    // and silently, until the app was next opened by hand.
    scheduleLinkCheck()

    guard let vaultPath = Self.vaultDirectory() else {
      task.setTaskCompleted(success: false)
      return
    }

    let work = DispatchWorkItem {
      let checked = vaultPath.withCString { websave_run_due_checks($0) }
      // Negative is a failure; zero just means nothing was due.
      task.setTaskCompleted(success: checked >= 0)
    }

    // The sweep is capped in Rust to finish inside the granted window, but iOS
    // can revoke that window early. Cancelling here stops the work item
    // reporting completion after the task has already expired, which the
    // scheduler counts against the app.
    task.expirationHandler = {
      work.cancel()
      task.setTaskCompleted(success: false)
    }

    DispatchQueue.global(qos: .utility).async(execute: work)
  }

  /// The same directory the Dart side opens.
  ///
  /// main.dart derives it from path_provider's application support directory
  /// plus "vault"; on iOS that is Library/Application Support inside the
  /// container. Resolved natively here because the handler has no engine to
  /// ask, so the two derivations have to agree by construction.
  private static func vaultDirectory() -> String? {
    FileManager.default
      .urls(for: .applicationSupportDirectory, in: .userDomainMask)
      .first?
      .appendingPathComponent("vault")
      .path
  }
}
