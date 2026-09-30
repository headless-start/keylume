/** The app version from package.json (set by vite.config.ts). */
declare const __APP_VERSION__: string;

/** Images bundled by Vite: their URL. */
declare module "*.svg" {
  const url: string;
  export default url;
}
