"""Extract the BoringCrypto reference docx into a Stem source file.

Goals (mapped to the user's 1:1 target):
- Title block for the cover.
- TOC marker (`section[id:toc]`) after the cover.
- Heading outline (numbered iff the reference uses numPr).
- Body paragraphs (drops TOC entries and field plumbing).
- Tables with captions attached from the following Caption paragraph.
- Images (one per <w:drawing>) with captions, written out to disk so
  the relative path resolves at render time.
- Hyperlinks: anchor + external (via word/_rels/document.xml.rels).
"""
import xml.etree.ElementTree as ET
import zipfile
import re
import os
import shutil
from pathlib import Path

W = '{http://schemas.openxmlformats.org/wordprocessingml/2006/main}'
A = '{http://schemas.openxmlformats.org/drawingml/2006/main}'
P_DRAW = '{http://schemas.openxmlformats.org/drawingml/2006/picture}'
R = '{http://schemas.openxmlformats.org/officeDocument/2006/relationships}'
REL_NS = '{http://schemas.openxmlformats.org/package/2006/relationships}'
ns = {'w': W[1:-1], 'a': A[1:-1], 'pic': P_DRAW[1:-1], 'r': R[1:-1]}

REF = Path('references/docx/paper_boringcrypto_security_policy.docx')
OUT_DIR = Path('references/docx/.extracted/boringcrypto_extract')
OUT_DIR.mkdir(parents=True, exist_ok=True)
ASSETS_DIR = OUT_DIR / 'assets'
ASSETS_DIR.mkdir(exist_ok=True)

z = zipfile.ZipFile(REF)
doc = ET.parse(z.open('word/document.xml')).getroot()
body = doc.find(W+'body')

# Build rId → target lookup for hyperlinks + images.
rels_doc = ET.parse(z.open('word/_rels/document.xml.rels')).getroot()
rel_map = {}
for r in rels_doc:
    rel_map[r.get('Id')] = (r.get('Type'), r.get('Target'))


def escape_text_body(s: str) -> str:
    bad = set('()[]{}@\\"')
    if any(c in bad for c in s):
        esc = s.replace('\\', '\\\\').replace('"', '\\"')
        return f'"{esc}"'
    return s


def get_run_text(r) -> str:
    parts = []
    for child in r:
        if child.tag == W+'t':
            parts.append(child.text or '')
        elif child.tag == W+'tab':
            parts.append(' ')
        elif child.tag == W+'br':
            parts.append(' ')
    return ''.join(parts)


def paragraph_has_drawing(p) -> bool:
    return p.find('.//' + W+'drawing') is not None


def extract_first_image(p):
    """Walk paragraph; return (rid, alt) for the first embedded image."""
    blip = p.find('.//' + A + 'blip')
    if blip is None:
        return None
    rid = blip.get(R+'embed')
    if not rid:
        return None
    # Try to find a doc-pr title/descr for alt text.
    docPr = p.find('.//{http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing}docPr')
    alt = (docPr.get('descr') if docPr is not None else None) or (docPr.get('title') if docPr is not None else None) or 'figure'
    return (rid, alt)


def render_inline_pieces(parent) -> str:
    """Build a stem-text body string from a paragraph's inline content,
    converting hyperlinks to @link[...]. Returns the body fragment to
    place inside `p(...)` etc."""
    parts = []
    for child in parent:
        if child.tag == W+'r':
            txt = get_run_text(child)
            if txt:
                parts.append(stem_escape(txt))
        elif child.tag == W+'hyperlink':
            anchor = child.get(W+'anchor')
            rid = child.get(R+'id')
            label_parts = []
            for sub in child.iter(W+'t'):
                label_parts.append(sub.text or '')
            label = ''.join(label_parts).strip()
            if not label:
                continue
            esc_label = label.replace('"', '\\"').replace('(', '\\(').replace(')', '\\)')
            if anchor:
                parts.append(f'@link[to:"#{anchor}"]({esc_label})')
            elif rid and rid in rel_map:
                target = rel_map[rid][1]
                parts.append(f'@link[to:"{target}"]({esc_label})')
            else:
                parts.append(label)
    text = ''.join(parts).strip()
    return text


def stem_escape(s: str) -> str:
    """Escape text-body literal so the stem parser doesn't choke.
    `(`/`)`/`{`/`}`/`[`/`]`/`@`/`\\` need backslash escapes inside a
    bare body. The caller decides whether to wrap with quotes."""
    return (
        s.replace('\\', '\\\\')
         .replace('(', '\\(')
         .replace(')', '\\)')
         .replace('[', '\\[')
         .replace(']', '\\]')
         .replace('{', '\\{')
         .replace('}', '\\}')
         .replace('@', '\\@')
    )


def cell_text(tc) -> str:
    parts = []
    for t in tc.iter(W+'t'):
        parts.append(t.text or '')
    s = ' '.join(parts)
    return re.sub(r'\s+', ' ', s).strip()


def table_to_stem(tbl, caption=None, indent=0):
    sp = '  ' * indent
    props = ['border:all']
    if caption:
        # `caption` becomes a stem string prop; backslash-escape quotes.
        esc = caption.replace('\\', '\\\\').replace('"', '\\"')
        props.append(f'caption:"{esc}"')
    out = [f'{sp}table[{", ".join(props)}]{{']
    rows = tbl.findall(W+'tr')
    # Track per-column vMerge to skip continuation cells.
    for i, tr in enumerate(rows):
        is_header = (i == 0)
        out.append(f'{sp}  row{"[kind:header]" if is_header else ""}{{')
        for tc in tr.findall(W+'tc'):
            tcPr = tc.find(W+'tcPr')
            colspan = 1
            vmerge_val = None
            if tcPr is not None:
                gs = tcPr.find(W+'gridSpan')
                if gs is not None:
                    colspan = int(gs.get(W+'val', 1))
                vm = tcPr.find(W+'vMerge')
                if vm is not None:
                    vmerge_val = vm.get(W+'val') or 'continue'
            if vmerge_val == 'continue':
                # Our exporter synthesizes these. Don't emit.
                continue
            # Detect cell alignment: walk paragraphs inside the cell
            # and find the dominant <w:jc> value. Cells with a uniform
            # centered align are common for numeric/N-A columns.
            jc_vals = set()
            for p_in in tc.iter(W+'p'):
                jc = p_in.find(W+'pPr/'+W+'jc')
                if jc is not None:
                    jc_vals.add(jc.get(W+'val'))
                else:
                    jc_vals.add('left')  # default
            align = None
            if jc_vals == {'center'}:
                align = 'center'
            elif jc_vals == {'right'}:
                align = 'right'
            txt = cell_text(tc)
            cprops = []
            if colspan > 1:
                cprops.append(f'colspan:{colspan}')
            if align:
                cprops.append(f'align:{align}')
            # Detect rowspan by walking subsequent rows for vMerge=continue
            # at the same grid position (best-effort: count consecutive).
            # The OOXML data doesn't give us a direct rowspan, so we leave
            # rowspan to manual fixup pass.
            cprop_str = '[' + ', '.join(cprops) + ']' if cprops else ''
            body_txt = stem_escape(txt) if txt else ''
            if body_txt:
                out.append(f'{sp}    cell{cprop_str}({body_txt})')
            else:
                out.append(f'{sp}    cell{cprop_str}()')
        out.append(f'{sp}  }}')
    out.append(f'{sp}}}')
    return out


def emit_paragraph(p, caption_consumes=False):
    """Return list of stem source lines for this paragraph, or [] to
    skip it."""
    pPr = p.find(W+'pPr')
    pStyle = pPr.find(W+'pStyle') if pPr is not None else None
    sty = pStyle.get(W+'val') if pStyle is not None else None

    # Drop pre-generated TOC entries and field machinery.
    if sty and (sty.startswith('TOC') or sty in (
        'TOCHeading', 'ContentsHeading', 'TableofFigures')):
        return []
    if any('TOC' in (it.text or '') or 'PAGEREF' in (it.text or '')
           for it in p.iter(W+'instrText')):
        return []

    # SEQ-only paragraphs (figure/table counter) are usually inside captions —
    # we'll consume captions as table/image attributes, so allow them through.

    # Detect inline page break: <w:br w:type="page"/>.
    page_break_run = p.find('.//' + W + 'br' + '[@' + W + 'type="page"]')
    text = render_inline_pieces(p)
    if not text:
        # Inline page break → emit pagebreak marker.
        if page_break_run is not None:
            return ['pagebreak']
        # Otherwise an empty paragraph — preserve it for spacing.
        # (skip if it's a Caption with no text, which is just SEQ chrome)
        if (sty == 'Caption') or any(it.text and 'SEQ' in it.text
                                     for it in p.iter(W+'instrText')):
            return []
        return ['p()']

    # Decide stem element from style.
    if sty == 'Title':
        return [f'title({escape_text_body(text)})']

    if sty and sty.startswith('Heading'):
        m = re.match(r'Heading(\d)', sty)
        if m:
            n = int(m.group(1))
            numPr = pPr.find(W+'numPr') if pPr is not None else None
            num_attr = 'numbered:true' if numPr is not None else ''
            bk = p.find(W+'bookmarkStart')
            id_attr = ''
            if bk is not None:
                bk_name = bk.get(W+'name', '')
                if not bk_name.startswith('_'):
                    id_attr = f'id:"{bk_name}"'
            props = ', '.join(x for x in [id_attr, num_attr] if x)
            propblk = f'[{props}]' if props else ''
            return [f'h{n}{propblk}({escape_text_body(text)})']

    # Captions are handled by the surrounding loop, not emitted as
    # standalone p().
    if sty == 'Caption':
        return []  # consumed by the surrounding walker

    # Default body paragraph.
    if not text.strip():
        return []
    return [f'p({escape_text_body(text)})']


def caption_text_for(p):
    """If paragraph is a Caption style, return its non-field text
    (the descriptive part after 'Table N – ' or 'Figure N – ')."""
    pPr = p.find(W+'pPr')
    pStyle = pPr.find(W+'pStyle') if pPr is not None else None
    sty = pStyle.get(W+'val') if pStyle is not None else None
    if sty != 'Caption':
        return None
    # Extract text. The SEQ field becomes literal "1", "2" in the run
    # output; the user-facing text starts after " – " or " - ".
    txt = ''.join(t.text or '' for t in p.iter(W+'t')).strip()
    # Strip the "Table N" / "Figure N" prefix since our exporter
    # auto-prefixes those.
    m = re.match(r'^(?:Table|Figure)\s+\d+\s*[–-]\s*(.*)$', txt)
    if m:
        return m.group(1).strip()
    return txt or None


# --- Walk the body, attaching captions to the previous tbl/figure ---

stem_lines = []
items = list(body)
i = 0
while i < len(items):
    el = items[i]
    if el.tag == W+'p':
        # Image paragraph?
        if paragraph_has_drawing(el):
            img_info = extract_first_image(el)
            if img_info:
                rid, alt = img_info
                target = rel_map.get(rid, ('', ''))[1]
                if target:
                    src_path = 'word/' + target.replace('\\', '/').lstrip('./')
                    # Save image to assets dir.
                    if src_path in z.namelist():
                        ext = os.path.splitext(src_path)[1] or '.bin'
                        local_name = f'image_{rid}{ext}'
                        local_path = ASSETS_DIR / local_name
                        with open(local_path, 'wb') as f:
                            f.write(z.read(src_path))
                        # Peek next sibling for Caption.
                        caption = None
                        if i + 1 < len(items) and items[i+1].tag == W+'p':
                            caption = caption_text_for(items[i+1])
                        # image[src:".../local_name", alt:"..", caption:".."]
                        esc_alt = alt.replace('"', '\\"')
                        rel = str(Path('references/docx/.extracted/boringcrypto_extract/assets') / local_name)
                        props = [f'src:"{rel}"', f'alt:"{esc_alt}"']
                        if caption:
                            esc_cap = caption.replace('\\', '\\\\').replace('"', '\\"')
                            props.append(f'caption:"{esc_cap}"')
                        stem_lines.append(f'image[{", ".join(props)}]')
                        if caption:
                            i += 1  # consume the caption paragraph too
                        i += 1
                        continue
        # Regular paragraph (also handles those that are captions but
        # weren't consumed by a preceding tbl/figure).
        lines = emit_paragraph(el)
        stem_lines.extend(lines)
    elif el.tag == W+'tbl':
        # Look ahead for a Caption.
        caption = None
        if i + 1 < len(items) and items[i+1].tag == W+'p':
            caption = caption_text_for(items[i+1])
        stem_lines.extend(table_to_stem(el, caption=caption))
        if caption:
            i += 1  # consume the caption paragraph
    elif el.tag == W+'sectPr':
        pass
    i += 1

# Document metadata: page size + margins from the doc-level sectPr.
sectPr = body.find(W+'sectPr')
metadata = []
if sectPr is not None:
    pgSz = sectPr.find(W+'pgSz')
    pgMar = sectPr.find(W+'pgMar')
    if pgSz is not None:
        w = int(pgSz.get(W+'w', '11906'))
        h = int(pgSz.get(W+'h', '16838'))
        if w == 12240 and h == 15840:
            metadata.append('page-size:letter')
    if pgMar is not None:
        t = int(pgMar.get(W+'top', 1440))
        r = int(pgMar.get(W+'right', 1440))
        b = int(pgMar.get(W+'bottom', 1440))
        l = int(pgMar.get(W+'left', 1440))
        def tw_to_in(v): return f'{v/1440:g}in'
        if t == r == b == l:
            metadata.append(f'margin:{tw_to_in(t)}')
        else:
            metadata.append(f'margin:"{tw_to_in(t)} {tw_to_in(r)} {tw_to_in(b)} {tw_to_in(l)}"')

# Assemble final source.

final = []
if metadata:
    final.append('[' + ', '.join(metadata) + ']')
    final.append('')

# Header/footer blocks from the reference doc. Inspect each part XML
# for non-empty text; only emit the blocks that carry content.
def part_text(name):
    try:
        x = z.read(f'word/{name}.xml').decode('utf-8')
    except KeyError:
        return ''
    import re
    return ' '.join(t for t in re.findall(r'<w:t[^>]*>([^<]*)</w:t>', x)).strip()

# Map sectPr's footer references to part names by type.
hf_parts = {}
if sectPr is not None:
    for kind in ['header', 'footer']:
        for r in sectPr.findall('w:' + kind + 'Reference', ns):
            t = r.get(W+'type') or 'default'
            rid = r.get(R+'id')
            if rid and rid in rel_map:
                target = rel_map[rid][1]
                # target is like "header2.xml" — strip extension
                base = target.replace('.xml','').replace('/','').replace('\\','')
                hf_parts[(kind, t)] = base

# Emit footer/header blocks at top of body (before TOC).
hf_lines = []
for (kind, scope), base in hf_parts.items():
    txt = part_text(base)
    if not txt:
        continue
    # Detect PAGE/NUMPAGES fields in this part to translate.
    raw = z.read(f'word/{base}.xml').decode('utf-8')
    has_page = ' PAGE ' in raw or 'InstrPAGE' in raw or '>PAGE<' in raw
    has_numpages = 'NUMPAGES' in raw
    # If the text has page-counter fields, swap their literal value
    # (often a stale rendered number like "1") for our inline elements.
    body_text = txt
    if has_page:
        body_text = re.sub(r'Page\s+\d+', 'Page @page-number()', body_text)
    if has_numpages:
        body_text = re.sub(r'of\s+\d+', 'of @total-pages()', body_text)
    # Escape for stem text body.
    esc = body_text.replace('"', '\"')
    scope_attr = '' if scope == 'default' else f'[scope:{scope}]'
    hf_lines.append(f'{kind}{scope_attr}{{ p("{esc}") }}')

# Prepend the H/F lines after metadata, before the first content.
if hf_lines:
    final.append('')
    final.extend(hf_lines)
    final.append('')


# Inject pagebreak + TOC + pagebreak between the cover and the body
# (first non-title, non-image line that starts a Heading).
inserted_toc = False
for j, line in enumerate(stem_lines):
    if not inserted_toc and (line.startswith('h1') or line.startswith('h2')):
        final.append('')
        final.append('pagebreak')
        final.append('')
        final.append('section[id:toc]')
        final.append('')
        final.append('pagebreak')
        final.append('')
        inserted_toc = True
    final.append(line)

print('\n'.join(final))
