import { join } from "node:path";

export function processUser(id: string): string {
  return formatEmail(id);
}

function formatEmail(s: string): string {
  return s.toLowerCase();
}

export class UserService {
  constructor(public name: string) {}

  greet(): string {
    return processUser(this.name);
  }
}

const ROUTE = "/users";
