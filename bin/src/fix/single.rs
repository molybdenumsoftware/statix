use std::{borrow::Cow, convert::TryFrom};

use lib::Report;
use rnix::{Root, TextSize, WalkEvent};

use crate::{err::SingleFixErr, fix::Source, utils};

pub struct SingleFixResult<'δ> {
    pub src: Source<'δ>,
}

fn pos_to_byte(line: usize, col: usize, src: &str) -> Result<TextSize, SingleFixErr> {
    let mut byte: TextSize = TextSize::of("");
    for (l, _) in src
        .split_inclusive('\n')
        .zip(1..)
        .take_while(|(_, i)| *i < line)
    {
        byte += TextSize::of(l);
    }
    byte += TextSize::try_from(col).map_err(|_| SingleFixErr::Conversion(col))?;

    if usize::from(byte) >= src.len() {
        Err(SingleFixErr::OutOfBounds(line, col))
    } else {
        Ok(byte)
    }
}

fn find(offset: TextSize, src: &str) -> Result<Report, SingleFixErr> {
    // we don't really need the source to form a completely parsed tree
    let parsed = Root::parse(src);
    let lints = utils::lint_map();

    parsed
        .syntax()
        .preorder_with_tokens()
        .filter_map(|event| match event {
            WalkEvent::Enter(child) => lints.get(&child.kind()).map(|rules| {
                rules
                    .iter()
                    .filter_map(|rule| rule.validate(&child))
                    .find(|report| report.total_suggestion_range().is_some())
            }),
            WalkEvent::Leave(_) => None,
        })
        .flatten()
        .find(|report| report.total_diagnostic_range().unwrap().contains(offset))
        .ok_or(SingleFixErr::NoOp)
}

pub fn single(line: usize, col: usize, src: &str) -> Result<SingleFixResult<'_>, SingleFixErr> {
    let mut src = Cow::from(src);
    let offset = pos_to_byte(line, col, &src)?;
    let report = find(offset, &src)?;

    report.apply(src.to_mut());

    Ok(SingleFixResult { src })
}
