import { TRASH_GRACE_DAYS } from "../utils";
import { TrashIcon } from "./Icons";

interface Props {
  count: number;
  autoPurge: boolean;
  onEmptyTrash: () => void;
}

/**
 * Explains the trash policy in plain language, so nobody is surprised when a
 * link disappears: dead links stay here, keep getting health-checked, and
 * come back automatically if the site recovers.
 */
export default function TrashBanner({
  count,
  autoPurge,
  onEmptyTrash,
}: Props) {
  return (
    <div className="trash-banner">
      <span className="trash-banner-icon">
        <TrashIcon size={18} />
      </span>
      <div className="trash-banner-text">
        <strong>
          Pages a server said are gone{count > 0 ? ` (${count})` : ""}
        </strong>
        <span>
          Only links whose server actively answered “not found” (404/410) land
          here. Anything we merely couldn't reach — a site that's down, or a
          Tailscale/VPN/LAN address you're not currently connected to — is
          listed under <strong>Unreachable</strong> instead and is never
          deleted. These are still re-checked, and leave the trash by
          themselves if the page comes back.
          {autoPurge
            ? ` Anything still gone after ${TRASH_GRACE_DAYS} days is deleted permanently.`
            : " Automatic deletion is turned off in Settings, so nothing is removed until you empty the trash."}
        </span>
      </div>
      {count > 0 && (
        <button className="btn btn-danger" onClick={onEmptyTrash}>
          Empty trash
        </button>
      )}
    </div>
  );
}
