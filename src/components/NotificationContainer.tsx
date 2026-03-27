import React from "react";
import Notification from "./Notification";

interface NotificationItem {
  id: number;
  type: "success" | "error" | "info";
  message: string;
}

interface NotificationContainerProps {
  notifications: NotificationItem[];
  removeNotification: (id: number) => void;
}

const NotificationContainer: React.FC<NotificationContainerProps> = ({
  notifications,
  removeNotification,
}) => {
  return (
    <div className="fixed top-15 right-5 z-[9999] flex flex-col items-end space-y-2">
      {notifications.map((n) => (
        <Notification
          key={n.id}
          type={n.type}
          message={n.message}
          onClose={() => removeNotification(n.id)}
        />
      ))}
    </div>
  );
};

export default NotificationContainer;
