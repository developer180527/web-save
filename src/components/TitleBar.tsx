import { getCurrentWindow } from "@tauri-apps/api/window";
import { MaximizeIcon, MinimizeIcon, XIcon } from "./Icons";

const IS_MAC = navigator.userAgent.includes("Mac");

/**
 * Custom title bar. On macOS the native traffic lights overlay our chrome
 * (TitleBarStyle::Overlay), so we only leave room for them; on
 * Windows/Linux the window is frameless and we render the controls.
 * Closing hides the window — the engine lives on in the tray.
 */
export default function TitleBar() {
  const win = getCurrentWindow();

  return (
    <header className={`titlebar ${IS_MAC ? "mac" : ""}`} data-tauri-drag-region>
      <span className="titlebar-title" data-tauri-drag-region>
        WebSave
      </span>
      {!IS_MAC && (
        <div className="titlebar-controls">
          <button
            className="titlebar-btn"
            title="Minimize"
            onClick={() => win.minimize()}
          >
            <MinimizeIcon size={15} />
          </button>
          <button
            className="titlebar-btn"
            title="Maximize"
            onClick={() => win.toggleMaximize()}
          >
            <MaximizeIcon size={13} />
          </button>
          <button
            className="titlebar-btn titlebar-close"
            title="Close"
            onClick={() => win.close()}
          >
            <XIcon size={15} />
          </button>
        </div>
      )}
    </header>
  );
}
