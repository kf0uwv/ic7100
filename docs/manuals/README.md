# Manuals

## `IC-7100_Full_Manual.pdf`

The primary source, from Icom America (`support/manual/2288`, file
`IC-7100_ENG_FM_5.pdf`). **Section 20, CONTROL COMMAND**, is the CI-V
reference: frame format at 20-2, the command table at 20-3 through 20-10,
and the data content description at 20-11.

The IC-7100 has no standalone "CI-V Reference Guide" — searching for one
finds the IC-R15's, which is a different radio. Its CI-V documentation
lives in this full manual.

## `ci-v-section.txt`

Not in git — neither this nor the PDF is ours to redistribute. Regenerate
it after downloading the manual. **Section 20 is PDF pages 360-376** (that
is the printed section 20, not page 20), and `-layout` matters: without it
the two-column tables interleave into nonsense.

```sh
pdftotext -layout -f 360 -l 376 IC-7100_Full_Manual.pdf ci-v-section.txt
```

The copy this repo was written against started a little earlier (mid
section 19) and so is slightly larger; nothing in the CI-V reference is
lost by starting at 360.

Section 20 as text, for grepping while writing command definitions. The
PDF is authoritative; this is a convenience and its two-column layout does
not always survive extraction cleanly. **Check the PDF before writing any
command**, per this repo's ADR 0001 and the discipline `ts570d`'s
`kenwood.md` and `ft991a`'s `yaesu.md` already enforce.
