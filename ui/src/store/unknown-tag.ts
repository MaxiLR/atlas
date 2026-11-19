/**
 * Ends a switch over a protocol union. Passing anything but `never` fails
 * type-checking, so a tag the store does not handle cannot be added to the
 * protocol unnoticed; at run time it is a core newer than the interface.
 */
export function unknownTag(value: never): never {
  throw new Error(`unknown tag in ${JSON.stringify(value)}`);
}
