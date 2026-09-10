export function getCurrentWindow() {
  return {
    label: "main",
    async minimize() {
      /* web: no window chrome */
    },
    async maximize() {
      /* web */
    },
    async unmaximize() {
      /* web */
    },
    async close() {
      window.close();
    },
    async hide() {
      /* web */
    },
    async show() {
      /* web */
    },
    async unminimize() {
      /* web */
    },
    async isMinimized() {
      return false;
    },
    async isMaximized() {
      return false;
    },
    async onResized(_cb: () => void) {
      return () => undefined;
    },
  };
}
