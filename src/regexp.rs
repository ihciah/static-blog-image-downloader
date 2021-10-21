use std::collections::{HashMap, HashSet};

use regex::Regex;

pub struct RegexWrapper {
    regex: Regex,
}

struct Replacer<'a>(&'a HashMap<String, String>);

impl Default for RegexWrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl RegexWrapper {
    pub fn new() -> Self {
        let regex = Regex::new(r"!\[.*?\]\((http[^\s)]*)\s*.*?\)").unwrap();
        Self { regex }
    }

    pub fn collect_urls(&self, contents: String, hashset: &mut HashSet<String>) {
        let matches = self.regex.captures_iter(&contents);
        for mat in matches {
            let m = mat.get(1).unwrap();
            hashset.insert(m.as_str().to_string());
        }
    }

    pub fn replace_urls(
        &self,
        contents: String,
        mapping: &HashMap<String, String>,
    ) -> String {
        let replacer = Replacer(mapping);
        self.regex.replace_all(&contents, replacer).to_string()
    }
}

impl<'a> regex::Replacer for Replacer<'a> {
    fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
        let base = caps.get(0).unwrap();
        let replaced = caps.get(1).unwrap();

        match self.0.get(replaced.as_str()) {
            Some(r) => {
                dst.push_str(&base.as_str()[..replaced.start() - base.start()]);
                dst.push_str(r);
                dst.push_str(&base.as_str()[replaced.end() - base.start()..]);
            }
            None => {
                // we will keep the original link
                dst.push_str(base.as_str());
                tracing::error!("replacing {} failed", replaced.as_str());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    struct Replacer;
    impl regex::Replacer for Replacer {
        fn replace_append(&mut self, caps: &regex::Captures<'_>, dst: &mut String) {
            let base = caps.get(0).unwrap();
            let replaced = caps.get(1).unwrap();
            println!(
                "{},{},{},{}",
                base.start(),
                base.end(),
                replaced.start(),
                replaced.end()
            );
            dst.push_str(&base.as_str()[..replaced.start() - base.start()]);
            dst.push_str("THE_HASH_URL");
            dst.push_str(&base.as_str()[replaced.end() - base.start()..]);
        }
    }
}
