use eyre::eyre;
use itertools::Itertools;
use smithay::input::keyboard::{KeysymHandle, XkbContextHandler};
use std::collections::{HashMap, HashSet};
use xkbcommon::xkb::{self, Keysym};

bitflags::bitflags! {
    /// Represents `xkb_mod_mask_t`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
    pub struct ModMask: u32 {
        const SHIFT   = 1 << 0;
        const LOCK    = 1 << 1;
        const CONTROL = 1 << 2;
        const MOD1    = 1 << 3;
        const MOD2    = 1 << 4;
        const MOD3    = 1 << 5;
        const MOD4    = 1 << 6;
        const MOD5    = 1 << 7;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Key {
    pub modmask: ModMask,
    pub keysym: Keysym,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySeq(Vec<Key>);

impl From<Vec<Key>> for KeySeq {
    fn from(keys: Vec<Key>) -> Self {
        Self(keys)
    }
}

impl KeySeq {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(vec![])
    }

    pub fn extract(keysym_handle: &KeysymHandle<'_>) -> Self {
        fn get(keysym_handle: &KeysymHandle<'_>, s: &str) -> bool {
            // Safety: We don't clone `&xkb::State` here.
            unsafe {
                keysym_handle
                    .xkb()
                    .state()
                    .mod_name_is_active(s, xkb::STATE_MODS_EFFECTIVE)
            }
        }

        // It would be nice to use `xkb::State.serialize_mods`, but it is not guaranteed that the indice are fixed.
        // (Actually, they are fixed. See `builtin_mods` in xkbcommon/libxkbcommon/src/keymap-priv.c.)
        // We can get the indice by `xkb::Keymap.mod_get_index`, but we don't have a keymap at the timing of the definition/creation of `ModMask`.
        let mut modmask = ModMask::default();
        modmask.set(ModMask::SHIFT, get(keysym_handle, xkb::MOD_NAME_SHIFT));
        modmask.set(ModMask::LOCK, get(keysym_handle, "Lock"));
        modmask.set(ModMask::CONTROL, get(keysym_handle, xkb::MOD_NAME_CTRL));
        modmask.set(ModMask::MOD1, get(keysym_handle, "Mod1"));
        modmask.set(ModMask::MOD2, get(keysym_handle, "Mod2"));
        modmask.set(ModMask::MOD3, get(keysym_handle, "Mod3"));
        modmask.set(ModMask::MOD4, get(keysym_handle, "Mod4"));
        modmask.set(ModMask::MOD5, get(keysym_handle, "Mod5"));

        keysym_handle
            .modified_syms()
            .iter()
            .map(|&keysym| Key { modmask, keysym })
            .collect_vec()
            .into()
    }

    pub fn as_keys(&self) -> &Vec<Key> {
        &self.0
    }

    pub fn as_keys_mut(&mut self) -> &mut Vec<Key> {
        &mut self.0
    }

    pub fn into_vec(self) -> Vec<Key> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn push(&mut self, key: Key) {
        self.0.push(key);
    }

    pub fn pop(&mut self) {
        self.0.pop();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeySeqWithoutShiftMask(KeySeq);

impl From<KeySeq> for KeySeqWithoutShiftMask {
    fn from(xs: KeySeq) -> Self {
        let mut xs = xs;
        for x in xs.as_keys_mut() {
            x.modmask.remove(ModMask::SHIFT);
        }

        Self(xs)
    }
}

pub struct KeySeqSerde {
    map: HashMap<String, ModMask>,
}

impl KeySeqSerde {
    pub fn new(map: HashMap<String, ModMask>) -> Self {
        Self { map }
    }

    pub fn kbd(&self, s: &str) -> eyre::Result<KeySeq> {
        s.split(' ')
            .map(|x| self.kbd_aux(x))
            .collect::<eyre::Result<Vec<_>>>()
            .map(KeySeq)
    }

    fn kbd_aux(&self, s: &str) -> eyre::Result<Key> {
        let mut cs = s.split('-').collect_vec();

        let Some(key) = cs.pop() else {
            return Err(eyre!("must not length zero: {}", s));
        };
        let keysym = xkb::keysym_from_name(key, xkb::KEYSYM_NO_FLAGS);
        // FYI, xkb::Keysym::NoSymbol doesn't exist.
        if keysym == xkb::keysyms::KEY_NoSymbol.into() {
            return Err(eyre!("No such keysym: {} in {}", key, s));
        }

        let mut modmask = ModMask::default();
        let mut seen = HashSet::new();
        for c in cs {
            if !seen.insert(c) {
                return Err(eyre!("prefix must appear at most one time: {} in {}", c, s));
            }

            if let Some(&m) = self.map.get(c) {
                modmask |= m;
            } else {
                return Err(eyre!("invaild prefix: {} in {}", c, s));
            }
        }

        Ok(Key { modmask, keysym })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use big_s::S;
    use rstest::rstest;

    fn nomod(keysym: Keysym) -> Key {
        let modmask = ModMask::default();
        Key { modmask, keysym }
    }

    fn control(keysym: Keysym) -> Key {
        let modmask = ModMask::CONTROL;
        Key { modmask, keysym }
    }

    fn mod1(keysym: Keysym) -> Key {
        let modmask = ModMask::MOD1;
        Key { modmask, keysym }
    }

    fn mod4(keysym: Keysym) -> Key {
        let modmask = ModMask::MOD4;
        Key { modmask, keysym }
    }

    fn mod5(keysym: Keysym) -> Key {
        let modmask = ModMask::MOD5;
        Key { modmask, keysym }
    }

    fn control_mod1(keysym: Keysym) -> Key {
        let modmask = ModMask::CONTROL | ModMask::MOD1;
        Key { modmask, keysym }
    }

    #[rstest(
        s, res,
        case("a", &[nomod(Keysym::a)]),
        case("A", &[nomod(Keysym::A)]),
        case("C-a", &[control(Keysym::a)]),
        case("C-A", &[control(Keysym::A)]),
        case("M-a", &[mod1(Keysym::a)]),
        case("M-A", &[mod1(Keysym::A)]),
        case("s-a", &[mod4(Keysym::a)]),
        case("s-A", &[mod4(Keysym::A)]),
        case("H-a", &[mod5(Keysym::a)]),
        case("H-A", &[mod5(Keysym::A)]),
        case("C-M-a", &[control_mod1(Keysym::a)]),
        case("C-M-A", &[control_mod1(Keysym::A)]),
        case("b", &[nomod(Keysym::b)]),
        #[should_panic]
        case("invalidkeysym", &[]),
        #[should_panic]
        case("invalidprefix-a", &[]),
        case("Return", &[nomod(Keysym::Return)]),
        #[should_panic]
        case("RETURN", &[]),
        case("a b", &[nomod(Keysym::a), nomod(Keysym::b)]),
        case("C-a M-b", &[control(Keysym::a), mod1(Keysym::b)]),
    )]
    #[trace]
    fn test_keyseq_serde_kbd(s: &str, res: &[Key]) {
        let keyseq_serde = KeySeqSerde::new(hashmap! {
            S("C") => ModMask::CONTROL,
            S("M") => ModMask::MOD1,
            S("s") => ModMask::MOD4,
            S("H") => ModMask::MOD5,
        });
        assert_eq!(keyseq_serde.kbd(s).unwrap().as_keys(), res);
    }

    #[rstest(
        s, res,
        #[should_panic]
        case("s-a", &[mod4(Keysym::a)]),
        #[should_panic]
        case("H-a", &[mod5(Keysym::a)]),
    )]
    #[trace]
    fn test_keyseq_serde_kbd_prefix_not_available(s: &str, res: &[Key]) {
        let keyseq_serde = KeySeqSerde::new(hashmap! {
            S("C") => ModMask::CONTROL,
            S("M") => ModMask::MOD1,
        });
        assert_eq!(keyseq_serde.kbd(s).unwrap().as_keys(), res);
    }
}
