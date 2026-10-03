# ezjww documentation

`ezjww` provides Python and TypeScript APIs for reading JWW and supported JWC
drawings, creating new JWW drawings, and converting them to DXF.

| Guide | Use it to |
| --- | --- |
| [Getting started](../README.md) | Install the package, read a drawing, convert files, and render previews |
| [API reference](JWC_API.md) | Choose JWW/JWC readers and understand source data, DXF results, and errors |
| [JWC support and limits](JWC_FORMAT.md) | Check supported geometry, coordinate systems, and rendering defaults |
| [Diagnostics](DIAGNOSTICS.md) | Handle stable issue codes and decoding warnings |
| [JWW signature](JWW_SIGNATURE.md) | Detect the JWW binary prefix |
| [JWW writing (Python/TypeScript/Rust)](JWW_WRITE.md) | Generate version-700 drawings with basic geometry, text and layer settings |
| [JWW native compatibility](JWW_COMPATIBILITY.md) | Check the recorded native matrix, memo/text changes, display evidence and Windows verification limits |
| [TypeScript package](../packages/ezjww/README.md) | Read, create and convert byte buffers in Node.js |
| [Browser example](../packages/ezjww/examples/browser/README.md) | Preview and convert local files, or generate a JWW example with WebAssembly |

Documentation in a repository checkout describes that checkout. For an installed
release, consult the corresponding release tag, especially when using newly
added formats or API options.
