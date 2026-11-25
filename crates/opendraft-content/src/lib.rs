//! The Content Manager's data side: the `.odpack` envelope, the
//! `opendraft.blocks` payload and the local library, with the hub client to
//! follow (docs/openhub-specification.md, section 13.1). No egui.

pub mod blocks_payload;
pub mod envelope;
pub mod library;

#[cfg(test)]
mod test_support;
