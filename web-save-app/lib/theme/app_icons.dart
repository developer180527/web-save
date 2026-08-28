// One icon vocabulary, drawn from whichever set belongs on the platform.
//
// Material glyphs on an iPhone are the clearest tell that an app was not
// written for it — the weights are heavier, the corners rounder, and they sit
// beside San Francisco badly. Cupertino glyphs on Android read as equally
// foreign, so this switches rather than picking a winner.
//
// Every icon the app uses is named here, so a call site never reaches for
// `Icons.` or `CupertinoIcons.` directly and the two sets cannot drift apart.
// Where Cupertino has no close equivalent the Material glyph is kept
// deliberately, and said so — a wrong-meaning icon is worse than a
// foreign-looking one.

import 'dart:io' show Platform;

import 'package:flutter/cupertino.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:flutter/material.dart';

/// Apple platforms get Cupertino glyphs; everything else gets Material.
///
/// macOS is included: SF Symbols are the system vocabulary there too, and the
/// desktop app sits beside Finder and Safari rather than beside Android.
bool get _isApple => !kIsWeb && (Platform.isIOS || Platform.isMacOS);

abstract final class AppIcons {
  // ---- navigation and structure ----
  static IconData get bookmark =>
      _isApple ? CupertinoIcons.bookmark : Icons.bookmark_outline;
  static IconData get inbox =>
      _isApple ? CupertinoIcons.tray : Icons.inbox_outlined;
  static IconData get tag => _isApple ? CupertinoIcons.tag : Icons.tag;
  static IconData get settings =>
      _isApple ? CupertinoIcons.slider_horizontal_3 : Icons.tune;
  static IconData get transfer =>
      _isApple ? CupertinoIcons.arrow_up_arrow_down : Icons.swap_vert;
  static IconData get globe => _isApple ? CupertinoIcons.globe : Icons.language;
  static IconData get search => _isApple ? CupertinoIcons.search : Icons.search;

  // ---- link health ----
  //
  // These carry meaning, not decoration, so the pairs are chosen for matching
  // sense first and matching shape second.
  static IconData get healthy =>
      _isApple ? CupertinoIcons.check_mark_circled : Icons.check_circle_outline;
  static IconData get changed => _isApple
      ? CupertinoIcons.arrow_2_circlepath
      : Icons.change_circle_outlined;
  static IconData get redirected =>
      _isApple ? CupertinoIcons.arrow_turn_up_right : Icons.turn_right;
  static IconData get dead =>
      _isApple ? CupertinoIcons.xmark_circle : Icons.cancel_outlined;
  static IconData get unchecked =>
      _isApple ? CupertinoIcons.question_circle : Icons.help_outline;
  static IconData get offline =>
      _isApple ? CupertinoIcons.cloud_download : Icons.cloud_off_outlined;
  static IconData get unreachable =>
      _isApple ? CupertinoIcons.globe : Icons.public_off;
  static IconData get reachable =>
      _isApple ? CupertinoIcons.globe : Icons.public;

  // ---- actions ----
  static IconData get add => _isApple ? CupertinoIcons.add : Icons.add;
  static IconData get remove => _isApple ? CupertinoIcons.minus : Icons.remove;
  static IconData get close => _isApple ? CupertinoIcons.xmark : Icons.close;
  static IconData get check =>
      _isApple ? CupertinoIcons.check_mark : Icons.check;
  static IconData get done => _isApple ? CupertinoIcons.check_mark : Icons.done;
  static IconData get refresh =>
      _isApple ? CupertinoIcons.refresh : Icons.refresh;
  static IconData get delete =>
      _isApple ? CupertinoIcons.delete : Icons.delete_outline;
  static IconData get edit =>
      _isApple ? CupertinoIcons.pencil : Icons.edit_outlined;
  static IconData get openExternal =>
      _isApple ? CupertinoIcons.arrow_up_right_square : Icons.open_in_new;
  static IconData get share =>
      _isApple ? CupertinoIcons.share : Icons.ios_share;
  static IconData get more =>
      _isApple ? CupertinoIcons.ellipsis : Icons.more_horiz;
  static IconData get paste =>
      _isApple ? CupertinoIcons.doc_on_clipboard : Icons.content_paste;
  static IconData get folder =>
      _isApple ? CupertinoIcons.folder_open : Icons.folder_open;
  static IconData get markUnread => _isApple
      ? CupertinoIcons.envelope_badge
      : Icons.mark_email_unread_outlined;
  static IconData get importDone =>
      _isApple ? CupertinoIcons.tray_arrow_down : Icons.download_done;

  // ---- favourites ----
  static IconData get starFilled =>
      _isApple ? CupertinoIcons.star_fill : Icons.star;
  static IconData get starOutline =>
      _isApple ? CupertinoIcons.star : Icons.star_outline;

  // ---- view modes ----
  static IconData get listView =>
      _isApple ? CupertinoIcons.list_bullet : Icons.view_list;
  static IconData get gridView =>
      _isApple ? CupertinoIcons.square_grid_2x2 : Icons.grid_view;

  // ---- window controls ----
  //
  // Only drawn on the frameless platforms; macOS uses its own traffic lights,
  // so these never appear on an Apple device and stay Material throughout.
  static IconData get windowMinimize => Icons.remove;
  static IconData get windowMaximize => Icons.crop_square;
  static IconData get windowClose => Icons.close;

  // ---- misc ----
  static IconData get brokenImage =>
      _isApple ? CupertinoIcons.photo : Icons.broken_image_outlined;
  static IconData get code => _isApple
      ? CupertinoIcons.chevron_left_slash_chevron_right
      : Icons.data_object;
}
