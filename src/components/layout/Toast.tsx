import { useEffect, useState } from "react";
import { useToastStore } from "@/stores/uiStore";

/** The prototype's transient message, kept outside `#root` so it never clips. */
export function Toast() {
  const message = useToastStore((state) => state.message);
  const token = useToastStore((state) => state.token);
  const [visible, setVisible] = useState(false);

  useEffect(() => {
    const host = document.getElementById("toast");
    if (!host) return;
    host.textContent = message;
    host.classList.toggle("show", Boolean(message));
    setVisible(Boolean(message));
  }, [message, token]);

  useEffect(() => {
    const host = document.getElementById("toast");
    if (host && !visible) host.classList.remove("show");
  }, [visible]);

  return null;
}
