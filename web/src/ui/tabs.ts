import { state } from "../state";
import type { SidebarTab } from "../state";

export function switchTab(tabName: SidebarTab): void {
  state.activeSidebarTab = tabName;

  // Update tab buttons
  document.querySelectorAll(".sidebar-tab").forEach((btn) => {
    const el = btn as HTMLElement;
    const isActive = el.dataset.tab === tabName;
    el.classList.toggle("active", isActive);
    el.setAttribute("aria-selected", String(isActive));
  });

  // Update tab panels
  document.querySelectorAll(".tab-panel").forEach((panel) => {
    const el = panel as HTMLElement;
    const isTarget = el.id === `tab-${tabName}`;
    el.classList.toggle("active", isTarget);
    el.hidden = !isTarget;
  });
}

export function bindTabs(): void {
  document.querySelectorAll(".sidebar-tab").forEach((btn) => {
    btn.addEventListener("click", () => {
      const tabName = (btn as HTMLElement).dataset.tab as SidebarTab;
      if (tabName) {
        switchTab(tabName);
      }
    });
  });
}
