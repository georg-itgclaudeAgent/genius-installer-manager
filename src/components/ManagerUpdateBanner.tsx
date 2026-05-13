import React from "react";

interface ManagerUpdateBannerProps {
  newVersion: string;
  onApply: () => void;
  applying: boolean;
}

export const ManagerUpdateBanner: React.FC<ManagerUpdateBannerProps> = ({
  newVersion,
  onApply,
  applying,
}) => (
  <div className="manager-update-banner">
    <span>
      Manager update ready ({newVersion})
    </span>
    <button onClick={onApply} disabled={applying}>
      {applying ? "Applying…" : "Restart to update"}
    </button>
  </div>
);
