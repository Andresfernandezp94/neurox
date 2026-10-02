// HomePage — landing pública de neurox.
//
// PRESENTA EL PRODUCTO. No es la app interna: esa vive detrás del login
// (StatusPanel, ChatPanel, ConfigViewer).
//
// Regla dura: este componente NO lee el store, no hace fetch y no toca
// /health ni ningún endpoint. Una landing que muestra "0 agentes · 12ms
// de latencia" filtra estado interno de la instalación antes de que
// nadie se autentique, y confunde el producto con el panel de control.
//
// Estructura: header · hero · main (capacidades) · footer.
// Único dato que consume es `version` — que se pasa como prop desde
// App.tsx y es pública por definición.

import { Card } from "../shared/components/molecules/Card";
import { AppLogo } from "../shared/components/AppLogo";
import { ThemeToggle } from "../shared/components/ThemeToggle";
import { SectionHeader } from "../shared/components/SectionHeader";
import {
  IconCpu,
  IconIntegrations,
  IconLogin,
  IconRobot,
  IconZap,
} from "../shared/components/Icons";

export interface HomePageProps {
  /** Versión del daemon, para el footer. Pública. */
  version?: string;
  /** Entra a la app interna. Dispara el flujo de login si hace falta. */
  onEnter?: () => void;
}

const CAPABILITIES = [
  {
    icon: IconRobot,
    title: "Agentes por sesión",
    text: "Cada sesión de chat obtiene su propio subproceso con memoria de trabajo aislada. Una sesión ocupada o caída nunca bloquea a otra.",
  },
  {
    icon: IconZap,
    title: "Tools engine",
    text: "Herramientas nativas con streaming de razonamiento, resultados estructurados, aprobaciones y sandbox por ruta.",
  },
  {
    icon: IconIntegrations,
    title: "Plugins y MCP",
    text: "Descubrimiento, registro y salud de plugins que publican sus herramientas al mismo registro del daemon.",
  },
  {
    icon: IconCpu,
    title: "Proveedores LLM",
    text: "Registro, credenciales y modelos por proveedor, incluidos modelos locales servidos por Ollama.",
  },
];

export function HomePage({ version, onEnter }: HomePageProps) {
  return (
    <div className="home" data-testid="home">
      {/* La grilla NO va acá: es una capa global en App.tsx, compartida
          por landing, login y panel. Ver <GridBackdrop fixed />. */}

      {/* ─── Header ─── */}
      <header className="home__header">
        <div className="home__brand">
          <AppLogo className="home__brand-logo" />
          <span className="home__brand-name">neurox</span>
        </div>

        {/* Toda la navegación vive en este grupo, pegado a la derecha:
            brand solo | links · theme · entrar. */}
        <div className="home__nav">
          <nav aria-label="Principal" className="home__nav-links">
            <a className="home__nav-link" href="#capacidades">
              Capacidades
            </a>
            <a className="home__nav-link" href="#arquitectura">
              Arquitectura
            </a>
          </nav>
          <ThemeToggle className="home__nav-theme" />
          {onEnter && (
            /* Sólo ícono, sin borde — misma clase que el theme toggle.
             * El texto "Entrar" queda como aria-label para lectores de
             * pantalla: visualmente es un botón de icono, pero sigue
             * siendo un control con nombre accesible. Se usa <button>
             * plano en vez de <Button> para no heredar `.btn`, que
             * pinta fondo y borde. */
            <button
              type="button"
              className="icon-btn-flat"
              onClick={onEnter}
              aria-label="Entrar al panel"
              title="Entrar al panel"
              data-testid="home-enter"
            >
              <IconLogin />
            </button>
          )}
        </div>
      </header>

      {/* El contenido vive dentro de <main>: es el único scroller de la
       * página (header y footer son fijos), así que las secciones se
       * mueven con el scroll.

       * El hero queda oculto por ahora (decisión del usuario): la landing
       * arranca directo en las capacidades y la arquitectura. El marcado
       * y sus estilos permanecen en git por si hay que volver atrás. */}
      <main className="home__main">

        {/* ─── Capacidades ─── */}
        <section className="home__section" id="capacidades">
          <SectionHeader
            title="Capacidades"
            description="Lo que el daemon expone y el panel administra."
          />
          <div className="home__cards">
            {CAPABILITIES.map(({ icon: Icon, title, text }) => (
              <Card className="home__card" key={title}>
                <span className="home__card-icon">
                  <Icon />
                </span>
                <h3 className="home__card-title">{title}</h3>
                <p className="home__card-text">{text}</p>
              </Card>
            ))}
          </div>
        </section>

        <section className="home__section" id="arquitectura">
          <SectionHeader
            title="Arquitectura"
            description="Un núcleo, varios clientes."
          />
          <div className="home__cards">
            <Card className="home__card">
              <h3 className="home__card-title">El daemon es el núcleo</h3>
              <p className="home__card-text">
                Toda la lógica de estado — sesiones, proveedores, agentes,
                credenciales, modelos — vive en el daemon. Corre como
                servicio local en 127.0.0.1:7878.
              </p>
            </Card>
            <Card className="home__card">
              <h3 className="home__card-title">Los clientes son ligeros</h3>
              <p className="home__card-text">
                Esta interfaz web y el sidebar de escritorio son thin
                clients: consumen la misma API y comparten los mismos datos,
                sin duplicar lógica.
              </p>
            </Card>
            <Card className="home__card">
              <h3 className="home__card-title">Aislamiento real</h3>
              <p className="home__card-text">
                Cada sesión spawnea su propio subproceso agente, así que
                varias sesiones trabajan en paralelo con memoria aislada.
              </p>
            </Card>
          </div>
        </section>
      </main>

      {/* ─── Footer ─── */}
      <footer className="home__footer">
        <div className="home__footer-row">
          <span className="muted text-sm">
            neurox — daemon local en 127.0.0.1:7878
          </span>
          {version && <span className="muted text-sm">v{version}</span>}
        </div>
      </footer>
    </div>
  );
}