import path from "path";

export function processUser(id) {
  return formatEmail(id);
}

function formatEmail(s) {
  return s.toLowerCase();
}

export class UserService {
  constructor(name) { this.name = name; }
  greet() { return processUser(this.name); }
}
