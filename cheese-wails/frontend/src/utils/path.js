export function shortenPath(path) {
  if (!path) return '';
  return path.replace(/^\/home\/[^/]+/, '~');
}
