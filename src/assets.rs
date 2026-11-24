//! What slides draw with besides their HTML: fonts for big text and
//! pictures for `<img>`. Loaded once with the deck.

use crate::fonts::Fonts;
use crate::images::Images;

#[derive(Clone, Debug, Default)]
pub struct Assets {
    pub fonts: Fonts,
    pub images: Images,
}
