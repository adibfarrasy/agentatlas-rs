; agentatlas HTML tags — minimal: capture id attributes as section defs so a page appears in the
; map; script/style blocks are handled by their own grammars elsewhere.
(attribute
  (attribute_name) @name
  (#eq? @name "id")) @definition.section