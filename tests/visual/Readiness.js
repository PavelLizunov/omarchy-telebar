// Wait for asynchronous local Images as well as the consumer's own state.
function pending(item) {
  if (item.source !== undefined && item.status === 2) return true // Image.Loading
  for (var child of item.children || []) if (pending(child)) return true
  return false
}
