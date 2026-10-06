function named(item, name) {
  if (item.objectName === name) return item
  for (var child of item.children || []) {
    var found = named(child, name)
    if (found) return found
  }
  return null
}

function matchesRadius(item, name, expected) {
  var found = named(item, name)
  return found !== null && found.radius === expected
}

// Wait for asynchronous local Images as well as the consumer's own state.
function pending(item) {
  if (item.source !== undefined && item.status === 2) return true // Image.Loading
  for (var child of item.children || []) if (pending(child)) return true
  return false
}
