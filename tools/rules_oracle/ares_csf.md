# Supplemental string tables

The optional `ares` feature composes the process string table after loading
`ra2md.csf`. Feature-off builds load only that required base table. The game
and the default `asset csf-get` / `csf-grep` route use the same
`assets::csf_file::CsfFile::load_active` owner. An explicit `--source` reads
one raw table. There is no fallback to `ra2.csf` in the active-table route.

Installed Ares 3.0 hook `6BD886 -> 10060DF0` tries `ares.csf`, then
`ares_<language-short>.csf`, then `stringtable00.csf` through
`stringtable99.csf` in ascending order. The base table's language selects
`us`, `uk`, `ge`, `fr`, `sp`, `it`, `ja`, `jb`, `ko`, or `ch`; unknown
languages select `us`. This is original `StringTable::GetLanguage@734640`
and the Ares fallback at `10060E13`, independent of the host locale.

`LoadAdditionalCSF@100608C0` requires a complete 24-byte header, signature
`0x43534620`, and a signed version of at least 2. The generic `ares.csf`
ignores language; all later tables must match the base language or use
`0xFFFFFFFF`. The base language and version remain authoritative. Missing
files and rejected headers are skipped, and zero supplemental header counts
do not prevent record parsing. Existing asset resolution owns loose/MIX
precedence; this mechanism adds no archive aliases or mod-specific paths.

`734A5F -> 10060C80` overrides labels case-insensitively, clearing their old
text before decoding its replacement. The original `ReadFile@734990`
commits each record immediately, and Ares ignores its failure return. A bad
suffix therefore retains prior writes and can leave an existing label with
a null value. The shared CSF header/value decoder preserves that bounded
case without an atomic file merge. Null values remain registered but do not
produce display text; existing missing-text handling is used.

[Three native histories](ares_csf.json), their
[metadata](ares_csf.meta.json), and their
[producer](ares_csf.py) cover layered append/override,
language/header rejection including signed versions and zero counts, and a
bad value marker after a valid append. The actual enumeration, header gates,
override/index hooks, record parser, normalization, search and sort execute.
Initialized base storage, file bytes/cursors, heap, formatting, ASCII strcmpi
and Syringe register transport are declared fixture boundaries. Rust tests
compare the loaded table and candidate order with those recorded outputs.

Native allocates 20,000 label/value slots and assumes one value per label.
Its binary search runs while new records are appended, before the file-end
sort. Arbitrary duplicate newly introduced keys, multiple values, oversized
labels/arrays, and truncated length or extra-data payloads are outside the
demonstrated malformed-body coverage; ordinary unique single-value tables
continue loading. This does not certify physical archive selection, complete
startup, all malformed inputs, or whole-mod compatibility. No RNG, simulation
timer or Detach mechanism participates in this presentation-only chain.
