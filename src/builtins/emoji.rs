// Secret emoji lookup — not documented, not in man page, not in cheatsheet.
// emoji("banana") → "🍌", emoji("rocket") → "🚀", unknown → passthrough.

/// Sorted by key for binary search.
static TABLE: &[(&str, &str)] = &[
    ("anchor", "⚓"),
    ("ant", "🐜"),
    ("apple", "🍎"),
    ("art", "🎨"),
    ("avocado", "🥑"),
    ("banana", "🍌"),
    ("battery", "🔋"),
    ("bean", "🫘"),
    ("bee", "🐝"),
    ("beer", "🍺"),
    ("bell", "🔔"),
    ("bird", "🐦"),
    ("bomb", "💣"),
    ("book", "📖"),
    ("boom", "💥"),
    ("bread", "🍞"),
    ("broccoli", "🥦"),
    ("bug", "🐛"),
    ("burger", "🍔"),
    ("butterfly", "🦋"),
    ("cactus", "🌵"),
    ("cake", "🎂"),
    ("candy", "🍬"),
    ("carrot", "🥕"),
    ("cat", "🐱"),
    ("cheese", "🧀"),
    ("cherry", "🍒"),
    ("chestnut", "🌰"),
    ("chocolate", "🍫"),
    ("cloud", "☁️"),
    ("clover", "☘️"),
    ("coconut", "🥥"),
    ("coffee", "☕"),
    ("cookie", "🍪"),
    ("corn", "🌽"),
    ("crab", "🦀"),
    ("crown", "👑"),
    ("cucumber", "🥒"),
    ("diamond", "💎"),
    ("dice", "🎲"),
    ("dog", "🐶"),
    ("dolphin", "🐬"),
    ("donut", "🍩"),
    ("drum", "🥁"),
    ("earth", "🌍"),
    ("egg", "🥚"),
    ("eggplant", "🍆"),
    ("fire", "🔥"),
    ("fish", "🐟"),
    ("flag", "🚩"),
    ("flower", "🌸"),
    ("fox", "🦊"),
    ("frog", "🐸"),
    ("garlic", "🧄"),
    ("gear", "⚙️"),
    ("grape", "🍇"),
    ("guitar", "🎸"),
    ("hammer", "🔨"),
    ("heart", "❤️"),
    ("herb", "🌿"),
    ("honey", "🍯"),
    ("key", "🔑"),
    ("kiwi", "🥝"),
    ("leaf", "🍃"),
    ("lemon", "🍋"),
    ("lettuce", "🥬"),
    ("light", "💡"),
    ("lightning", "⚡"),
    ("lock", "🔒"),
    ("mango", "🥭"),
    ("map", "🗺️"),
    ("melon", "🍈"),
    ("microscope", "🔬"),
    ("milk", "🥛"),
    ("moon", "🌙"),
    ("mountain", "⛰️"),
    ("mushroom", "🍄"),
    ("music", "🎵"),
    ("octopus", "🐙"),
    ("onion", "🧅"),
    ("orange", "🍊"),
    ("owl", "🦉"),
    ("peach", "🍑"),
    ("peanut", "🥜"),
    ("pear", "🍐"),
    ("penguin", "🐧"),
    ("pepper", "🌶️"),
    ("pie", "🥧"),
    ("pineapple", "🍍"),
    ("pizza", "🍕"),
    ("popcorn", "🍿"),
    ("potato", "🥔"),
    ("puzzle", "🧩"),
    ("rainbow", "🌈"),
    ("rice", "🍚"),
    ("rocket", "🚀"),
    ("rose", "🌹"),
    ("salt", "🧂"),
    ("scroll", "📜"),
    ("shield", "🛡️"),
    ("shrimp", "🦐"),
    ("snail", "🐌"),
    ("snake", "🐍"),
    ("snow", "❄️"),
    ("spider", "🕷️"),
    ("star", "⭐"),
    ("strawberry", "🍓"),
    ("sun", "☀️"),
    ("sunflower", "🌻"),
    ("sushi", "🍣"),
    ("sword", "⚔️"),
    ("taco", "🌮"),
    ("tea", "🍵"),
    ("telescope", "🔭"),
    ("tomato", "🍅"),
    ("tornado", "🌪️"),
    ("tree", "🌳"),
    ("trophy", "🏆"),
    ("tulip", "🌷"),
    ("turtle", "🐢"),
    ("volcano", "🌋"),
    ("water", "💧"),
    ("watermelon", "🍉"),
    ("wave", "🌊"),
    ("whale", "🐋"),
    ("wind", "💨"),
    ("wine", "🍷"),
    ("wrench", "🔧"),
];

pub fn lookup(word: &str) -> &str {
    let key = word.trim();
    // fast path: exact match via binary search
    if let Ok(i) = TABLE.binary_search_by_key(&key, |&(k, _)| k) {
        return TABLE[i].1;
    }
    // retry lowercased
    let lower = key.to_ascii_lowercase();
    if let Ok(i) = TABLE.binary_search_by_key(&lower.as_str(), |&(k, _)| k) {
        return TABLE[i].1;
    }
    // strip trailing 's' or 'es' for simple plurals (try both)
    if lower.ends_with('s') && lower.len() > 2 {
        let minus_s = &lower[..lower.len() - 1];
        if let Ok(i) = TABLE.binary_search_by(|&(k, _)| k.cmp(minus_s)) {
            return TABLE[i].1;
        }
        if lower.ends_with("es") && lower.len() > 3 {
            let minus_es = &lower[..lower.len() - 2];
            if let Ok(i) = TABLE.binary_search_by(|&(k, _)| k.cmp(minus_es)) {
                return TABLE[i].1;
            }
        }
    }
    word
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted() {
        for w in TABLE.windows(2) {
            assert!(w[0].0 < w[1].0, "out of order: {:?} >= {:?}", w[0].0, w[1].0);
        }
    }

    #[test]
    fn exact_match() {
        assert_eq!(lookup("banana"), "🍌");
        assert_eq!(lookup("rocket"), "🚀");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(lookup("Banana"), "🍌");
        assert_eq!(lookup("FIRE"), "🔥");
    }

    #[test]
    fn plural_strip() {
        assert_eq!(lookup("bananas"), "🍌");
        assert_eq!(lookup("cherries"), "cherries"); // irregular plural, passthrough
        assert_eq!(lookup("grapes"), "🍇");
        assert_eq!(lookup("tomatoes"), "🍅");
    }

    #[test]
    fn passthrough_unknown() {
        assert_eq!(lookup("xylophone"), "xylophone");
        assert_eq!(lookup(""), "");
    }
}
