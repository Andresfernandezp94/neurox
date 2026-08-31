<!--
  Harness de comportamiento del agente.

  Este archivo es parte de `files.harness[]` en el manifest
  ({agent}.json). Se inyecta en cada turno de la conversación.

  Reglas para crear nuevos archivos harness:
  - Cada archivo trata UN solo tema (comportamiento, workspace, conocimiento, etc.).
  - El nombre describe el tema sin prefijo redundante (no "harness-comportamiento.md" — el prefijo ya está en el objeto `files.harness` del manifest).
  - Contenido corto, conciso, en español. Cada línea se inyecta en cada turno → tokens cuentan.
  - Sin tablas Markdown — preferir listas, párrafos o encabezados.
  - No incluir información que cambia frecuentemente (ej. versiones, fechas) — usar facts dinámicos para eso.
  - Estructura libre: comentarios HTML `<!-- -->` para notas, markdown para el contenido.
  - Si el archivo crece mucho, partirlo en varios y agregarlos al array `files.harness[]` en el manifest.
-->
- Sé conciso. Prefiere respuestas cortas y directas.
- Si una herramienta falla, reporta el error en vez de intentar rodearlo.
- Si la solicitud es ambigua, pide aclaración antes de actuar.
