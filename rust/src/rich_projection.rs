use crate::projection::{array, integer, media_for, number, text, typed, yes};
use serde_json::{Value, json};

struct Rich<'a> {
    roots: &'a [String],
    blocks: Vec<Value>,
    nodes: usize,
    text: usize,
    entities: usize,
    truncated: bool,
}
impl Rich<'_> {
    fn admit(&mut self, depth: usize) -> bool {
        if depth > 16 || self.nodes == 0 || self.blocks.len() >= 256 {
            self.truncated = true;
            return false;
        }
        self.nodes -= 1;
        true
    }
    fn rich_text(&mut self, value: &Value, depth: usize) -> (String, Vec<Value>) {
        if !self.admit(depth) {
            return (String::new(), Vec::new());
        }
        let kind = text(&value["@type"], 64);
        if kind == "richTexts" {
            let mut body = String::new();
            let mut entities = Vec::new();
            let mut offset = 0usize;
            for child in array(&value["texts"], 1024) {
                let (part, ranges) = self.rich_text(child, depth + 1);
                for mut entity in ranges {
                    entity["offset"] = number(integer(&entity["offset"], 0) + offset as i128);
                    entities.push(entity);
                }
                offset += part.encode_utf16().count();
                body.push_str(&part);
                if self.nodes == 0 {
                    break;
                }
            }
            return (body, entities);
        }
        let (body, mut entities, name) = if value["text"].is_object() {
            let (body, entities) = self.rich_text(&value["text"], depth + 1);
            let name = match kind.as_str() {
                "richTextBold" => "bold",
                "richTextItalic" => "italic",
                "richTextUnderline" => "underline",
                "richTextStrikethrough" => "strikethrough",
                "richTextSpoiler" => "spoiler",
                "richTextFixed" => "code",
                "richTextUrl" | "richTextEmailAddress" | "richTextPhoneNumber" => "textUrl",
                "richTextMentionName" => "mentionName",
                "richTextMention" => "mention",
                "richTextHashtag" => "hashtag",
                "richTextCashtag" => "cashtag",
                "richTextBotCommand" => "botCommand",
                _ => "",
            };
            (body, entities, name)
        } else {
            let source = if kind == "richTextCustomEmoji" {
                &value["alternative_text"]
            } else {
                value.get("text").unwrap_or(&value["expression"])
            };
            let body = text(source, self.text);
            if source
                .as_str()
                .is_some_and(|s| s.chars().count() > body.chars().count())
            {
                self.truncated = true;
            }
            self.text -= body.chars().count();
            (
                body,
                Vec::new(),
                if kind == "richTextCustomEmoji" {
                    "customEmoji"
                } else {
                    ""
                },
            )
        };
        if !name.is_empty() && !body.is_empty() && self.entities > 0 {
            let mut entity = json!({"type":name,"offset":0,"length":body.encode_utf16().count()});
            match kind.as_str() {
                "richTextUrl" => entity["url"] = json!(text(&value["url"], 2048)),
                "richTextEmailAddress" => {
                    entity["url"] = json!(format!("mailto:{}", text(&value["email_address"], 2041)))
                }
                "richTextPhoneNumber" => {
                    entity["url"] = json!(format!("tel:{}", text(&value["phone_number"], 64)))
                }
                _ => {}
            }
            if name == "mentionName" {
                entity["userId"] = number(integer(&value["user_id"], 0));
            }
            if name == "customEmoji" {
                entity["customEmojiId"] = json!(integer(&value["custom_emoji_id"], 0).to_string());
            }
            entities.push(entity);
            self.entities -= 1;
        }
        (body, entities)
    }
    fn add_text(&mut self, value: &Value, depth: usize, style: &str) {
        let (body, entities) = self.rich_text(value, depth);
        if body.is_empty() || self.blocks.len() >= 256 {
            return;
        }
        self.blocks
            .push(json!({"kind":"text","text":body,"entities":entities,"style":style}));
    }
    fn caption(&mut self, value: &Value, depth: usize) {
        let cap = typed(value, "pageBlockCaption");
        for field in ["text", "credit"] {
            if let Some(v) = cap.get(field) {
                self.add_text(v, depth, "");
            }
        }
    }
    fn walk(&mut self, value: &Value, depth: usize) {
        if !self.admit(depth) {
            return;
        }
        let kind = text(&value["@type"], 64);
        let field = match kind.as_str() {
            "pageBlockTitle" => "title",
            "pageBlockSubtitle" => "subtitle",
            "pageBlockHeader" => "header",
            "pageBlockSubheader" => "subheader",
            "pageBlockSectionHeading"
            | "pageBlockParagraph"
            | "pageBlockPreformatted"
            | "pageBlockThinking"
            | "pageBlockExpandableBlockQuote"
            | "pageBlockPullQuote" => "text",
            "pageBlockKicker" => "kicker",
            "pageBlockFooter" => "footer",
            _ => "",
        };
        if !field.is_empty() {
            let style = if kind == "pageBlockPreformatted" {
                "pre"
            } else if matches!(
                kind.as_str(),
                "pageBlockTitle" | "pageBlockHeader" | "pageBlockSectionHeading"
            ) {
                "heading"
            } else {
                ""
            };
            self.add_text(&value[field], depth + 1, style);
            if let Some(credit) = value.get("credit") {
                self.add_text(credit, depth + 1, "");
            }
            return;
        }
        let media_kind = match kind.as_str() {
            "pageBlockPhoto" => "photo",
            "pageBlockVideo" => "video",
            "pageBlockAnimation" => "gif",
            "pageBlockAudio" => "audio",
            "pageBlockDocument" => "file",
            "pageBlockVoiceNote" => "voice",
            _ => "",
        };
        if !media_kind.is_empty() {
            let media = media_for(media_kind, value, self.roots);
            if !media.is_null() && crate::wire::truthy(&media["file"]) {
                self.blocks.push(json!({"kind":media_kind,"text":"","entities":[],"media":media,"spoiler":yes(&value["has_spoiler"])}));
            } else {
                self.blocks.push(json!({"kind":"unsupported","text":crate::content_projection::preview_label(media_kind),"entities":[]}));
            }
            self.caption(&value["caption"], depth + 1);
            return;
        }
        match kind.as_str() {
            "pageBlockCover" => self.walk(&value["cover"], depth + 1),
            "pageBlockList" => {
                for item in array(&value["items"], 256) {
                    let label = text(&item["label"], 32);
                    for (index, child) in array(&item["blocks"], 256).enumerate() {
                        let start = self.blocks.len();
                        self.walk(child, depth + 1);
                        if index == 0
                            && !label.is_empty()
                            && self.blocks.len() > start
                            && self.blocks[start]["kind"] == "text"
                        {
                            let entry = &mut self.blocks[start];
                            let prefix = format!("{label} ");
                            entry["text"] =
                                json!(format!("{prefix}{}", entry["text"].as_str().unwrap_or("")));
                            if let Some(ranges) = entry["entities"].as_array_mut() {
                                for entity in ranges {
                                    entity["offset"] = number(
                                        integer(&entity["offset"], 0)
                                            + prefix.encode_utf16().count() as i128,
                                    );
                                }
                            }
                        }
                        if self.nodes == 0 || self.blocks.len() >= 256 {
                            break;
                        }
                    }
                    if !self.admit(depth + 1) {
                        break;
                    }
                }
            }
            "pageBlockBlockQuote"
            | "pageBlockDetails"
            | "pageBlockEmbeddedPost"
            | "pageBlockCollage"
            | "pageBlockSlideshow" => {
                if let Some(header) = value.get("header") {
                    self.add_text(header, depth + 1, "heading");
                }
                for child in array(&value["blocks"], 256) {
                    self.walk(child, depth + 1);
                    if self.nodes == 0 || self.blocks.len() >= 256 {
                        break;
                    }
                }
                self.caption(&value["caption"], depth + 1);
                if let Some(credit) = value.get("credit") {
                    self.add_text(credit, depth + 1, "");
                }
            }
            "pageBlockTable" => {
                self.add_text(&value["caption"], depth + 1, "heading");
                for row in array(&value["cells"], 256) {
                    for cell in array(row, 32) {
                        self.add_text(&cell["text"], depth + 1, "");
                    }
                    if !self.admit(depth + 1) {
                        break;
                    }
                }
            }
            "pageBlockDivider" => self
                .blocks
                .push(json!({"kind":"divider","text":"","entities":[]})),
            "pageBlockAnchor" => {}
            _ => self.blocks.push(
                json!({"kind":"unsupported","text":"Unsupported block","entities":[],"type":kind}),
            ),
        }
    }
}

pub fn rich_message(value: &Value, roots: &[String]) -> Value {
    let message = typed(value, "richMessage");
    let mut rich = Rich {
        roots,
        blocks: Vec::new(),
        nodes: 1024,
        text: 16384,
        entities: 512,
        truncated: message["blocks"].as_array().is_some_and(|b| b.len() > 256),
    };
    for block in array(&message["blocks"], 256) {
        rich.walk(block, 0);
        if rich.nodes == 0 || rich.blocks.len() >= 256 {
            rich.truncated = true;
            break;
        }
    }
    let summary = rich
        .blocks
        .iter()
        .filter(|b| b["kind"] == "text")
        .filter_map(|b| b["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n\n")
        .chars()
        .take(16384)
        .collect::<String>();
    json!({"kind":"rich","text":summary,"entities":[],"blocks":rich.blocks,"full":yes(&message["is_full"]),"rtl":yes(&message["is_rtl"]),"truncated":rich.truncated})
}
