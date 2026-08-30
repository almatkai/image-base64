/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `base64-to-image` command */
  export type Base64ToImage = ExtensionPreferences & {}
  /** Preferences accessible in the `image-to-base64` command */
  export type ImageToBase64 = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `base64-to-image` command */
  export type Base64ToImage = {}
  /** Arguments passed to the `image-to-base64` command */
  export type ImageToBase64 = {}
}

