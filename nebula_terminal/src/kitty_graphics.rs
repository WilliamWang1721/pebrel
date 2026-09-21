use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

const MAX_RGBA_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum KittyEvent {
    Query { image_id: u32 },
    Image {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        columns: u16,
        rows: u16,
    },
}

#[derive(Default, Clone, Copy)]
enum Phase {
    #[default]
    Ground,
    Esc,
    Apc,
    ApcEsc,
}

#[derive(Default)]
pub struct KittyGraphics {
    phase: Phase,
    payload: Vec<u8>,
    interested: bool,
    transfer: Option<Transfer>,
}

struct Transfer {
    data: Vec<u8>,
    expected_base64: usize,
    width: u32,
    height: u32,
    columns: u16,
    rows: u16,
}

impl KittyGraphics {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<(usize, KittyEvent)> {
        let mut events = Vec::new();
        for (i, &byte) in bytes.iter().enumerate() {
            match self.phase {
                Phase::Ground if byte == 0x1b => self.phase = Phase::Esc,
                Phase::Ground => {},
                Phase::Esc if byte == b'_' => {
                    self.phase = Phase::Apc;
                    self.payload.clear();
                    self.interested = true;
                },
                Phase::Esc if byte != 0x1b => self.phase = Phase::Ground,
                Phase::Esc => {},
                Phase::Apc if byte == 0x1b => self.phase = Phase::ApcEsc,
                Phase::Apc => self.push(byte),
                Phase::ApcEsc if byte == b'\\' => {
                    if self.interested
                        && let Some(event) = self.parse()
                    {
                        events.push((i + 1, event));
                    }
                    self.phase = Phase::Ground;
                    self.payload.clear();
                    self.interested = false;
                },
                Phase::ApcEsc => {
                    self.push(0x1b);
                    self.push(byte);
                    self.phase = Phase::Apc;
                },
            }
        }
        events
    }

    fn push(&mut self, byte: u8) {
        if !self.interested {
            return;
        }
        if self.payload.is_empty() && byte != b'G' {
            self.interested = false;
            return;
        }
        self.payload.push(byte);
    }

    fn parse(&mut self) -> Option<KittyEvent> {
        let body = self.payload.strip_prefix(b"G")?;
        let split = body.iter().position(|&b| b == b';').unwrap_or(body.len());
        let (control, data) = body.split_at(split);
        let data = data.strip_prefix(b";").unwrap_or_default();
        let action = option(control, b'a').and_then(|value| value.first()).copied();
        let more = option_u32(control, b'm').unwrap_or(0) == 1;

        if action == Some(b'q')
            && option(control, b't') == Some(b"d")
            && option_u32(control, b'f') == Some(32)
        {
            return Some(KittyEvent::Query { image_id: option_u32(control, b'i')? });
        }

        if action.is_none() && self.transfer.is_some() {
            append(self.transfer.as_mut().unwrap(), data)?;
            return if more { None } else { finish(self.transfer.take().unwrap()) };
        }

        if action != Some(b'T')
            || option(control, b't') != Some(b"d")
            || option_u32(control, b'f') != Some(32)
        {
            return None;
        }

        let width = option_u32(control, b's')?;
        let height = option_u32(control, b'v')?;
        let expected = usize::try_from(width.checked_mul(height)?.checked_mul(4)?).ok()?;
        if expected > MAX_RGBA_BYTES {
            return None;
        }
        let expected_base64 = expected.div_ceil(3) * 4;
        let mut transfer = Transfer {
            data: Vec::new(),
            expected_base64,
            width,
            height,
            columns: option_u32(control, b'c')
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0),
            rows: option_u32(control, b'r')
                .and_then(|value| u16::try_from(value).ok())
                .unwrap_or(0),
        };
        append(&mut transfer, data)?;
        if more {
            self.transfer = Some(transfer);
            None
        } else {
            finish(transfer)
        }
    }
}

fn option(control: &[u8], key: u8) -> Option<&[u8]> {
    control
        .split(|&b| b == b',')
        .find_map(|part| part.strip_prefix(&[key, b'=']))
}

fn option_u32(control: &[u8], key: u8) -> Option<u32> {
    std::str::from_utf8(option(control, key)?).ok()?.parse().ok()
}

fn append(transfer: &mut Transfer, data: &[u8]) -> Option<()> {
    (transfer.data.len() + data.len() <= transfer.expected_base64)
        .then(|| transfer.data.extend_from_slice(data))
}

fn finish(transfer: Transfer) -> Option<KittyEvent> {
    let rgba = STANDARD.decode(transfer.data).ok()?;
    let expected = usize::try_from(
        transfer.width.checked_mul(transfer.height)?.checked_mul(4)?,
    )
    .ok()?;
    (rgba.len() == expected).then_some(KittyEvent::Image {
        rgba,
        width: transfer.width,
        height: transfer.height,
        columns: transfer.columns,
        rows: transfer.rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_direct_query() {
        let mut parser = KittyGraphics::default();
        assert_eq!(
            parser.feed(b"\x1b_Gf=32,a=q,t=d,i=42,s=1,v=1;AAAAAA==\x1b\\"),
            [(39, KittyEvent::Query { image_id: 42 })]
        );
    }

    #[test]
    fn joins_direct_image_chunks() {
        let mut parser = KittyGraphics::default();
        assert!(parser.feed(b"\x1b_Gf=32,a=T,t=d,s=1,v=1,c=2,r=3,m=1;AQID\x1b\\").is_empty());
        assert_eq!(
            parser.feed(b"\x1b_Gm=0;BA==\x1b\\"),
            [(
                13,
                KittyEvent::Image {
                    rgba: vec![1, 2, 3, 4],
                    width: 1,
                    height: 1,
                    columns: 2,
                    rows: 3,
                }
            )]
        );
    }
}
