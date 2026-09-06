declare module "node:fs" {
  export interface Dirent {
    name: string;
    isDirectory(): boolean;
  }
  export function readFileSync(path: string, encoding: string): string;
  export function readdirSync(
    path: string,
    options: { withFileTypes: true },
  ): Dirent[];
}

declare module "node:path" {
  export function resolve(...paths: string[]): string;
  export function join(...paths: string[]): string;
  export function relative(from: string, to: string): string;
  export function dirname(p: string): string;
}

declare module "node:url" {
  export function fileURLToPath(url: string | URL): string;
}
