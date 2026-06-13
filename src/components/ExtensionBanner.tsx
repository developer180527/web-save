import { openUrl } from "@tauri-apps/plugin-opener";
import { EXTENSION_INSTALL_URL } from "../utils";
import { ImportIcon, XIcon } from "./Icons";

interface Props {
  onDismiss: () => void;
}

/**
 * First-run nudge to install the browser extension. Shown only while the app
 * has never received a capture from it — phrased as an offer, since the app
 * cannot tell "not installed" from "installed but unused".
 */
export default function ExtensionBanner({ onDismiss }: Props) {
  return (
    <div className="ext-banner">
      <span className="ext-banner-icon">
        <ImportIcon size={18} />
      </span>
      <div className="ext-banner-text">
        <strong>Save pages straight from your browser</strong>
        <span>
          Add the WebSave extension to capture any page with a right-click. It
          talks to this app locally — nothing leaves your machine.
        </span>
      </div>
      <button
        className="btn btn-primary"
        onClick={() => openUrl(EXTENSION_INSTALL_URL)}
      >
        Get the extension
      </button>
      <button className="icon-btn" title="Dismiss" onClick={onDismiss}>
        <XIcon size={15} />
      </button>
    </div>
  );
}
