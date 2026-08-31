# neurox web (`neurox-web`)

Cliente web *thin* del daemon neurox. Chat en tiempo real, selector de
modelos/agentes y gestión de proveedores — todo consumiendo la API del daemon
en `127.0.0.1:7878`.

Stack: **React + Vite + TypeScript** (tests con Vitest).

## Requisitos

- Node 18+
- El daemon neurox corriendo (`http://127.0.0.1:7878`).

## Scripts

```bash
npm install
npm run dev            # dev server con HMR → http://localhost:5173
npm run build          # tsc --noEmit && vite build
npm run preview        # sirve el build en :5173
npm test               # vitest (watch)
npm run test:run       # vitest una pasada
npm run lint           # tsc --noEmit
```

## Estructura

```
src/
├── api/            ← clientes de la API del daemon
│   ├── sessions.ts   (createSession, sendMessage, streamMessage SSE)
│   ├── llm.ts        (proveedores, listModels — normaliza array de strings)
│   └── env.ts        (gestión de API keys)
├── components/
│   ├── ChatPanel.tsx / ChatFooter.tsx   (chat)
│   ├── ModelSelector.tsx                (dropdown de modelos por proveedor)
│   └── ProvidersPanel.tsx               (proveedores + API keys)
├── hooks/          ← useChatTabs, useWebSocket, useDefaultAgentId, ...
├── shared/         ← componentes de UI (design system atómico)
└── store/          ← estado global (sesiones, WS)
```

## Contrato con el daemon

- **Crear sesión**: `POST /v1/sessions` con `{agent_id}`.
- **Chat streaming**: `POST /v1/sessions/:id/messages/stream` con
  `{agent_id, text}`; respuesta SSE con eventos
  `{type: thinking|content|tool_call|tool_result|error}` y cierre por `[DONE]`.
- **Modelos**: `GET /v1/llm/providers/:id/models` devuelve un **array de
  strings**; `listModels` los normaliza a `{id}`.

## Convenciones de UI

- Sin `!important`. Usar el design system atómico (Card / Button / Input /
  Badge / IconButton + utilidades).
- La paleta de colores es estable (no cambiarla al alinear componentes).
- El sidebar de escritorio es la referencia visual; la web se alinea a él.

## Componente Rust (`server/`)

El directorio `server/` contiene un componente en Rust (con su `Cargo.lock`).
Sus artefactos de build (`target/`) están ignorados por git.
