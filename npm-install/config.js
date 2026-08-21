/**
 * Global configuration
 */
export const CONFIG = {
  /**
   * The name of the binary
   * @type {string}
   */
  name: "mob",

  /**
   * Where to save the unpacked files, relative to the package root
   * @type {string}
   */
  path: "./bin",

  /**
   * The URL to download the binary from
   *
   * - `{{bin_name}}` is the name declared above
   * - `{{triple}}` is the Rust target triple for this platform
   * - `{{version}}` is the version number as `0.0.0` (taken from package.json)
   *
   * @type {string}
   */
  url: "https://github.com/mobdotso/cli/releases/download/v{{version}}/{{bin_name}}-v{{version}}-{{triple}}.tar.gz",
};
