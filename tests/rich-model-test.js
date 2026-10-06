// Pure display contract for rich posts, including unsafe input rendering.
const fs = require('fs')
const vm = require('vm')
const assert = require('assert')
const M = {}
vm.createContext(M)
vm.runInContext(fs.readFileSync('app/Model.js', 'utf8').replace(/^\.pragma library\s*/, ''), M)
const photo = id => ({ kind: 'photo', text: '', media: { file: { id } } })
const message = { id: 27, chatId: 42, content: { kind: 'rich', text: 'Before After', blocks: [
  { kind: 'text', text: 'Before', entities: [] }, photo(50), photo(60), { kind: 'text', text: 'After', entities: [] }
] } }
const items = M.mediaMessages([message])
assert.strictEqual(items.length, 2)
assert.deepStrictEqual(Array.from(items, m => m.content.media.file.id), [50, 60])
assert(items.every(m => m.id === 27 && m.chatId === 42))
assert.strictEqual(message.content.kind, 'rich')
assert.strictEqual(M.contentLabel(message.content), '')
assert.strictEqual(M.previewOf(message), 'Before After')
const html = M.richText('<img src="file:///etc/passwd">', [
  { type: 'textUrl', offset: 0, length: 4, url: 'javascript:alert(1)' }
], true, 'transparent', null, '#ffffff')
assert(!html.includes('<img') && !html.includes('javascript:') && html.includes('&lt;'))
console.log('PASS: rich media identity, previews and safe rendering')
