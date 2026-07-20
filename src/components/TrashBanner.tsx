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
          Links that stopped responding{count > 0 ? ` (${count})` : ""}
        </strong>
        <span>
          They're still checked on the normal schedule — if a site comes back,
          it leaves the trash automatically and returns to your library.
          {autoPurge
            ? ` Anything still dead after ${TRASH_GRACE_DAYS} days is deleted permanently.`
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
