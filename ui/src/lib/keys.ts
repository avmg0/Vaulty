/** A label with its shortcut in brackets, if it has one: "Lock (Ctrl+L)". */
export const withKeys = (label: string, keys: string) => (keys ? `${label} (${keys})` : label);
