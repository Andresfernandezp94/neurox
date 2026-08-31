> **HISTORICAL**: este documento menciona `adan`/`eva` (nombres
> pre-rename). El agente canónico del workspace es `default`.
> Ver [EP-0009](../../changes/EP-0009-purge-non-default-agent-names/)
> para el rename completo. Para git history exacto:
> `git log --all --grep="adan|eva"`.

# Metodologias de Ingenieria de Software Modernas (2026)

> Investigacion realizada por EVA para Andres Fernandez P. — fecha 2026-08-09
>
> Fuentes oficiales consultadas: agilemanifesto.org, scrumguides.org, kanbanguides.org,
> basecamp.com/shapeup, martinfowler.com, scaledagile.com, lean.org, producttalk.org,
> domainlanguage.com (DDD).

---

## Indice

1. [Manifiesto Agile (base filosofica)](#1-manifiesto-agile)
2. [Scrum](#2-scrum)
3. [Kanban](#3-kanban)
4. [Extreme Programming (XP)](#4-extreme-programming-xp)
5. [Lean Software Development](#5-lean-software-development)
6. [Shape Up (Basecamp)](#6-shape-up-basecamp)
7. [Scaled Agile Framework (SAFe)](#7-scaled-agile-framework-safe)
8. [Domain-Driven Design (DDD)](#8-domain-driven-design-ddd)
9. [Test-Driven Development (TDD)](#9-test-driven-development-tdd)
10. [Behavior-Driven Development (BDD)](#10-behavior-driven-development-bdd)
11. [Continuous Integration / Delivery / Deployment (CI/CD)](#11-cicd)
12. [DORA / DevOps Research](#12-dora--devops)
13. [Continuous Discovery](#13-continuous-discovery)
14. [Spec-Driven Development (referencia)](#14-spec-driven-development-referencia)
15. [Mapa comparativo](#15-mapa-comparativo)

---

## 1. Manifiesto Agile

**Origen**: 2001, 17 autores (Kent Beck, Martin Fowler, Jeff Sutherland, Ken Schwaber, etc.).

**No es una metodologia, es el paraguas filosofico** que dio origen a Scrum, XP, FDD, DSDM, Crystal y a todos los frameworks que vinieron despues.

**4 valores**:

| Valor izquierdo (preferido) | Valor derecho (tambien tiene valor) |
|---|---|
| Individuos e interacciones | Procesos y herramientas |
| Software funcionando | Documentacion exhaustiva |
| Colaboracion con el cliente | Negociacion de contratos |
| Respuesta al cambio | Seguimiento de un plan |

**12 principios** (resumen): satisfaccion del cliente, bienvenida al cambio, entregas frecuentes, trabajo cercano, gente motivada, conversacion cara a cara, software funcionando como medida de progreso, desarrollo sostenible, excelencia tecnica, simplicidad, equipos autoorganizados, reflexion regular.

**Por que importa en 2026**: el 100% de los frameworks modernos nace de aqui. Si no se entiende el manifiesto, los frameworks se aplican como religion y no como herramienta.

---

## 2. Scrum

**Origen**: Ken Schwaber + Jeff Sutherland, 1995 (formalizado en 2010, ultima guia oficial 2020).

**Estructura canonica** (la guia dice que esto es inmutable — sacarle una pieza deja de ser Scrum):

| Elemento | Tipo | Descripcion |
|---|---|---|
| Sprint | Evento contenedor | 1 mes o menos, duracion fija |
| Sprint Planning | Evento | Por que es valioso + que se hace + como (max 8h) |
| Daily Scrum | Evento | 15 min, solo developers, sincronizacion |
| Sprint Review | Evento | Demo con stakeholders (max 4h) |
| Sprint Retrospective | Evento | Mejora del equipo (max 3h) |
| Product Backlog | Artefacto | Lista priorizada por el Product Owner |
| Sprint Backlog | Artefacto | Items del sprint + plan + Sprint Goal |
| Increment | Artefacto | Suma del trabajo "Done" del sprint |
| Product Owner | Rol | Maximiza valor, ordena backlog |
| Scrum Master | Rol | Facilita, quita impedimentos |
| Developers | Rol | Construyen el incremento |

**3 pilares empiricos**: transparencia, inspeccion, adaptacion.
**5 valores**: compromiso, foco, apertura, respeto, coraje.

**Limites reconocidos**: pensado para equipos pequenos (<=10 personas), un solo equipo, un solo producto. Escalar Scrum lleva a SAFe, LeSS, Nexus o Scrum@Scale.

**Por que sigue vivo en 2026**: simple de entender, estructura clara, muy bueno para productos con requisitos que cambian. Critica comun: se vuelve ceremony-driven si no hay Scrum Master fuerte.

---

## 3. Kanban

**Origen**: Adaptado del Toyota Production System por David Anderson (2007-2010). Guia oficial actualizada en 2025.

**Definicion oficial 2025** (mucho mas minimalista que las versiones anteriores):

> Kanban es una estrategia para optimizar el flujo de valor a traves de un proceso. Tiene 3 practicas:
> 1. Definir y visualizar el workflow
> 2. Gestionar activamente los items en el workflow
> 3. Mejorar el workflow

**Minimum Definition of Workflow (DoW)** — explicito y obligatorio:
1. Unidades de valor (work items)
2. Cuando un item empieza y termina
3. Estados por los que pasa
4. Como se controla el WIP (work in progress)
5. Politicas de transicion entre estados
6. **Service Level Expectation (SLE)**: pronostico (ej: "85% de items en 8 dias o menos")

**4 flow metrics obligatorias**:
- WIP
- Throughput
- Work Item Age
- Cycle Time

**Cambio importante 2025**: se elimino la palabra "inmutable" del Kanban. Ahora es explicito que puede personalizarse. Esto responde anos de critica de la comunidad.

**Diferencia clave con Scrum**:
- Scrum = cadencia fija (sprints), roles fijos, cambios de scope NO permitidos durante el sprint
- Kanban = flujo continuo, sin roles predefinidos, WIP limits en lugar de sprints, pull system

**Por que se usa en 2026**: equipos de soporte, mantenimiento, operaciones, infra. Muy bueno donde el trabajo es interrumpible y no se puede predecir el tamano.

---

## 4. Extreme Programming (XP)

**Origen**: Kent Beck, 1996-1999. Resurge cada vez que se publica un nuevo libro de Beck.

**5 valores**: comunicacion, simplicidad, retroalimentacion, coraje, respeto.

**Practicas canonicas** (las 12 originales):

| Practica | Resumen |
|---|---|
| Pair programming | 2 devs, 1 teclado |
| TDD | Tests antes que codigo |
| Continuous integration | Integrar varias veces al dia |
| Refactoring | Mejorar estructura sin cambiar comportamiento |
| Simple design | Hacer solo lo necesario hoy |
| Collective code ownership | Cualquiera puede tocar cualquier codigo |
| On-site customer | Cliente disponible en el equipo |
| Small releases | Releases pequenas y frecuentes |
| Planning game | Iteracion + planning en pareja (dev + cliente) |
| Sustainable pace | 40h/semana, sin crunch |
| Coding standards | Convencion comun del equipo |
| System metaphor | Narrativa compartida del sistema |

**Estado en 2026**: muchas practicas se adoptaron de forma aislada (TDD, CI, pair programming) en otros frameworks. XP "puro" no es comun, pero sus practicas son la base de la ingenieria moderna.

**Resurgimiento**: Kent Beck reescribio TDD en 2023 ("Canon TDD"), hubo reunion de los firmantes originales del Manifiesto Agile en 2024. XP esta volviendo al centro de la conversacion por la complejidad del codigo generado por IA.

---

## 5. Lean Software Development

**Origen**: Adaptado de Lean Manufacturing (Toyota Production System) por Mary + Tom Poppendieck (2003).

**7 principios del Lean Thinking** (LEI 2026):

1. **Propósito**: resolver el problema del cliente, no maximizar output
2. **Pensar en el sistema completo**: optimizar el flujo end-to-end, no etapas locales
3. **Crear flujo**: eliminar batching, esperas, handoffs
4. **Pull, no push**: el trabajo se jala cuando hay capacidad
5. **Mejora continua (Kaizen)**: nunca termina
6. **Respeto por las personas**: trabajadores saben mas del trabajo que los managers
7. **Estandarizar para innovar**: standards son la base sobre la que se experimenta

**7 desperdicios (muda)** en software:
1. Trabajo parcialmente hecho
2. Features extra
3. Reaprendizaje
4. Handoffs entre personas
5. Switching entre tareas
6. Esperas
7. Defectos

**Relacion con Agile**: Lean es la base filosofica. Scrum es una implementacion de Lean. Kanban es otra.

**Por que importa en 2026**: el DORA report 2025 confirma que las capacidades de plataforma + ingenieria siguen correlacionando con performance de entrega. Lean provee el marco mental.

---

## 6. Shape Up (Basecamp)

**Origen**: Ryan Singer (Basecamp), publicado como libro free en 2019, oficialmente publicado 2026.

**Anti-manifiesto**: Shape Up nacio como reaccion a los problemas que Singer veia en Scrum a escala: backlogs infinitos, sprints que no terminan, estimaciones que no muestran incertidumbre.

**Conceptos clave**:

| Concepto | Definicion |
|---|---|
| **Cycle** | 6 semanas fijas, igual para todos los equipos |
| **Cool-down** | 2 semanas entre cycles para bugs, ideas, betting table |
| **Shaping** | Trabajo de "darle forma" a ideas abstractas (no wireframes, no user stories) |
| **Pitch** | Documento: problema + appetite + solucion + rabbit holes + no-gos |
| **Betting table** | Reunion donde se decide que pitches se comprometen |
| **Appetite** | Cuanto tiempo QUIERO invertir (vs. estimate: cuanto CREO que toma) |
| **Hill chart** | Visualizacion: cada scope va de "subiendo" (unknowns) a "bajando" (ejecucion) |
| **Circuit breaker** | Si un proyecto no termina en 1 cycle, se CANCELA por default |
| **Scope hammering** | Cortar scope agresivamente cuando se vence el tiempo |
| **Breadboarding** | Boceto de UI sin estilo, solo conexiones entre elementos |

**Reglas duras**:
- Fixed time, variable scope (al reves de Scrum)
- No bugs en el backlog (se arreglan en cool-down)
- "Done" = deployado a produccion, no "listo para QA"
- 6 semanas es el techo absoluto, no se extiende

**Por que importa en 2026**: Basecamp (37signals) lo usa en produccion desde 2014. Equipos que se ahogan en Scrum lo adoptan para romper el ciclo de "nunca terminamos nada".

---

## 7. Scaled Agile Framework (SAFe)

**Origen**: Dean Leffingwell, 2011. Version actual 6.0 (2023), con AI-Native SAFe anunciado 2026.

**Que es**: NO es una metodologia. Es un **operating system para escalar Lean-Agile a toda la empresa**.

**4 configuraciones** (de menos a mas):
1. **Essential SAFe** — el nucleo (ARTs + PI Planning)
2. **Large Solution SAFe** — multiples ARTs coordinando
3. **Portfolio SAFe** — estrategia + portfolio
4. **Full SAFe** — todo lo anterior + lean portfolio management

**5 disciplinas core**:
1. **Lean Portfolio Management** — estrategia, financiacion, governance
2. **Team and Technical Agility** — equipos Agile + practicas de ingenieria
3. **Product Development Flow** — desde concepto a cash
4. **Large Solution Integration** — coordinacion de suppliers
5. **Leadership and Culture** — Lean-Agile Mindset

**Roles clave**: Release Train Engineer (RTE), Product Management, System Architect, Epic Owner, Business Owner, Scrum Master / Team Coach.

**Eventos clave**: PI Planning (2 dias, 50-100 personas), System Demo, Inspect & Adapt, Iteration.

**PI (Program Increment)**: 8-12 semanas tipicamente, 4-5 iterations de 2 semanas + 1 Innovation and Planning iteration.

**AI-Native SAFe (2026)**: la nueva adicion reorganiza PI Planning alrededor de outcomes (no outputs), agrega AI Value Architect, AI-Native Teams, PI Outcome Planning. Es un reconocimiento explicito de que AI cambia el modelo.

**Por que importa**: SAFe es el framework escalado mas usado (~30% del Fortune 100 lo usa formalmente). Critica principal: bureaucratic, "Scrum-but-with-more-ceremonies". Counter-argument: sin algo asi, multiples equipos no se coordinan.

---

## 8. Domain-Driven Design (DDD)

**Origen**: Eric Evans, 2003 ("Big Blue Book"). DDD Reference (2015) es el resumen.

**Que NO es**: una metodologia de proceso. No tiene sprints, roles, eventos.
**Que SI es**: un approach de diseño de software centrado en el modelo del dominio.

**Building blocks**:

| Concepto | Resumen |
|---|---|
| **Bounded context** | Limite explicito donde un modelo aplica |
| **Ubiquitous language** | Vocabulario comun devs + expertos de dominio |
| **Context map** | Como los bounded contexts se relacionan |
| **Entity** | Objetos con identidad |
| **Value object** | Objetos sin identidad, definidos por sus valores |
| **Aggregate** | Cluster de entidades + value objects con una raiz |
| **Domain event** | Algo que paso en el pasado |
| **Repository** | Abstraccion de persistencia para aggregates |
| **Service** | Logica de dominio que no encaja en una entidad |

**Strategic patterns** (context map): Partnership, Shared Kernel, Customer-Supplier, Conformist, Anti-Corruption Layer, Open-Host Service, Published Language, Separate Ways.

**Por que importa en 2026**: DDD es el lenguaje que une arquitectura (microservicios, hexagonal, clean arch) con diseño de software. Con el auge de microservicios y multi-tenancy, los bounded contexts son la forma natural de hablar de limites.

**Critica**: muchos devs aplican DDD superficialmente. Sin ubiquitous language real, es solo una jerga mas.

---

## 9. Test-Driven Development (TDD)

**Origen**: Kent Beck, ~1999-2002 dentro de XP. Re-formalizado por Beck en 2023 ("Canon TDD").

**Ciclo canonico** (Red → Green → Refactor):

1. **Red**: escribir un test que falla para la siguiente funcionalidad
2. **Green**: escribir el minimo codigo para que el test pase
3. **Refactor**: limpiar el codigo manteniendo tests verdes

**Por que funciona**:
- Tests son red de seguridad
- Fuerza a pensar en la interface antes que la implementacion
- Produce codigo testeable por diseño

**TDD vs Test-First vs Test-After**:
- TDD = test primero + refactor obligatorio
- Test-First = test primero, sin refactor formal
- Test-After = feature primero, test despues

**Variantes**:
- **ATDD (Acceptance Test-Driven Development)**: tests de aceptacion antes que codigo
- **BDD (Behavior-Driven Development)**: tests escritos en lenguaje natural (Given/When/Then)
- **Property-Based Testing**: en vez de ejemplos, propiedades que se verifican con datos random

**Estado 2026**: resurgimiento fuerte. Martin Fowler actualizo su articulo en dic-2023. La "Canon TDD" de Beck (2023) redefine las reglas para la era de los AI coding assistants. La idea: con LLMs, TDD se vuelve mas importante porque el codigo generado por AI necesita tests rigurosos.

---

## 10. Behavior-Driven Development (BDD)

**Origen**: Dan North, 2003-2006. Evolucion de TDD.

**Nucleo**: escribir tests en lenguaje de negocio, no en jerga tecnica.

**Formato Given/When/Then**:
```
Given un usuario con 100 USD en su cuenta
When intenta transferir 150 USD
Then la transaccion es rechazada
Then el balance sigue siendo 100 USD
```

**Frameworks**: Cucumber, SpecFlow, Behave, Gauge, JBehave.

**3 practicas (North)**:
1. Discovery: conversacion sobre ejemplos concretos antes de codigo
2. Formulation: convertir ejemplos a formato ejecutable
3. Automation: ejecutar los ejemplos como tests

**Diferencia con TDD**: BDD se enfoca en el **comportamiento** del sistema desde la perspectiva del usuario. TDD se enfoca en unidades tecnicas.

**Por que importa en 2026**: BDD es el puente entre product (PM, BA) y desarrollo. Cuando se hace bien, los criterios de aceptacion son tests ejecutables. Cuando se hace mal, es solo documentacion vestida de test.

---

## 11. CI/CD

**Origen**: Conceptos de XP (1999) + formalizacion por Jez Humble + David Farley ("Continuous Delivery" 2010). Martin Fowler reescribio su articulo de CI en 2024.

**3 niveles (no son lo mismo)**:

| Nivel | Que hace | Quien lo decide |
|---|---|---|
| **Continuous Integration (CI)** | Cada commit se integra + build + test | Practica tecnica del equipo |
| **Continuous Delivery (CD)** | Cada build que pasa tests puede ir a produccion con UN click | Practica tecnica + ops |
| **Continuous Deployment (CD)** | Cada build que pasa tests VA a produccion automatico | Negocio (o confianza extrema) |

**Las 11 practicas de CI** (Fowler 2024):
1. Todo en un mainline versionado
2. Build automatizado
3. Build self-testing
4. Todos commitean al mainline todos los dias
5. Cada push al mainline dispara un build
6. Builds rotos se arreglan inmediatamente
7. Build rapido (<10 min ideal)
8. Esconder WIP (feature flags, dark launches)
9. Test en un clon de produccion
10. Todos pueden ver que esta pasando
11. Deploy automatizado

**Deployment pipeline** (Humble/Farley):
- Commit stage: rapido, tests basicos
- Acceptance stage: mas lento, tests funcionales + performance
- Capacity stage: tests de carga
- Manual/auto deploy a prod

**Por que importa en 2026**: DORA report confirma que elite performers hacen deploys multiples por dia con <15min lead time. Sin CI/CD maduro, esto es imposible.

---

## 12. DORA / DevOps

**Origen**: Google Cloud + team de research academico. Lleva 10+ anos publicando el "State of DevOps Report".

**Que es**: NO es una metodologia. Es un **programa de investigacion** que mide que separa a equipos de alto rendimiento del resto.

**4 DORA metrics** (core, inmutables):

| Metrica | Que mide | Elite |
|---|---|---|
| **Deployment Frequency** | Cuantas veces deployas a prod | On-demand (multiples por dia) |
| **Lead Time for Changes** | Commit → produccion | < 1 hora |
| **Change Failure Rate** | % de deploys que rompen algo | 0-15% |
| **Failed Deployment Recovery Time** | Tiempo en volver a verde | < 1 hora |

**DORA Core Model (2024+)**: 33 capabilities agrupadas en 5 areas:
1. **Technical** — CI/CD, trunk-based, loose coupling
2. **Process** — WIP limits, code review, visibility
3. **Culture** — psychological safety, learning, blameless postmortems
4. **Architectural** — modular, evolvable
5. **AI-augmented** — uso de AI para productividad (nueva desde 2024)

**DORA 2025 findings** (el ultimo reporte):
- "AI acts as an amplifier" — la AI no reemplaza capacidades tecnicas, las potencia
- Las capabilities que correlacionan con performance cambian menos que nunca
- Platform engineering sigue siendo top capability
- "User-centricity" predict 40% higher performance (desde 2023, consistente)

**Por que importa**: DORA es la unica fuente con metodologia academica seria sobre que funciona. Si tenes que elegir una sola "metodologia" para empezar, las 4 metricas + las capabilities del Core Model son el ROI mas alto.

---

## 13. Continuous Discovery

**Origen**: Teresa Torres, "Continuous Discovery Habits" (2021). Construye sobre Marty Cagan (SVPG), Jeff Patton, y discovery lean.

**Que es**: disciplina de hacer discovery de producto **continuamente**, no en sprints separados.

**Practica central**: cada **product trio** (PM + designer + engineer) se reune **al menos 2 veces por semana con clientes** para:
1. Entrevistar (15-30 min)
2. Descubrir oportunidades
3. Testear suposiciones
4. Iterar

**Componentes clave**:
- **Opportunity Solution Tree** (visualizacion): outcome → opportunities → solutions → tests
- **Assumption testing**: identificar las suposiciones mas riesgosas y testearlas primero
- **Story-based interviewing**: no encuestas, conversaciones con historias reales
- **Opportunity backlog**: priorizado por impacto, no por quien grita mas

**Por que importa en 2026**: la mayoria de los frameworks anteriores asumen que ya tenes el producto y la vision. Continuous Discovery resuelve el problema previo: como saber **que construir** antes de commitment. Marty Cagan lo considera la pieza que le falta a la mayoria de los equipos Agile.

**Critica**: requiere clientes accesibles, tiempo de PM dedicado, y cultura que valore el discovery sobre el output. No funciona en empresas con culturas muy command-and-control.

---

## 14. Spec-Driven Development (referencia)

**Origen**: AI-native. OpenSpec (Amazon Kiro), spec-kit (GitHub), Tessl, Marty Cagan + AI practitioners.

**Definicion**: el agente AI lee una spec formal (proposal + design + requirements por repo) y produce el codigo siguiendo la spec. La spec es ejecutable y verificable.

**Stack de artefactos** (estilo OpenSpec usado en Sixbell):
1. `proposal.md` — por que (negocio, no tecnico)
2. `design.md` — como (tecnico, decisiones, trade-offs)
3. `specs/EP-NNNN-NN-*/requirements.md` — que (criterios de aceptacion, Kiro-compatible)
4. `tasks.md` — orden de ejecucion
5. ADRs (Architecture Decision Records) — inmutables

**Herramientas que lo soportan**:
- **Amazon Kiro** — IDE con specs como first-class artifact
- **GitHub spec-kit** — tooling para Spec-Driven Development
- **OpenSpec** (Sixbell) — metodologia transversal
- **Tessl** — specs como test framework

**Relacion con lo anterior**:
- Es **complementario** a TDD, DDD, CI/CD, DORA — no los reemplaza
- Encaja como la fase "como se implementa" en un workflow Agile o Shape Up
- Funciona especialmente bien combinado con Continuous Discovery (las specs codifican lo descubierto)

**Estado 2026**: emergente. Kiro lanzo en 2025, GitHub spec-kit es alpha. La tesis: con LLMs suficientemente capaces, la especificacion es el cuello de botella, no la escritura de codigo.

---

## 15. Mapa comparativo

### Por que existe (filosofia)

| Framework | Padre filosofico |
|---|---|
| Agile | Manifiesto Agile 2001 |
| Scrum, XP, FDD, DSDM | Agile (todos) |
| Kanban | Lean (Toyota) |
| Lean Software Dev | Toyota Production System |
| Shape Up | Agile + Lean + critica a Scrum |
| SAFe | Lean-Agile a escala empresarial |
| DDD | Domain-driven design (no es proceso) |
| TDD, BDD, ATDD | XP / Agile engineering |
| CI/CD | XP / DevOps |
| DORA | DevOps research |
| Continuous Discovery | Lean Startup + product management |
| Spec-Driven | AI-native (2025+) |

### Que tipo de problema resuelve

| Framework | Que problema ataca |
|---|---|
| Scrum | "Como coordinamos un equipo pequeno" |
| Kanban | "Como manejamos trabajo continuo sin cadencia" |
| XP | "Como escribimos codigo de alta calidad" |
| Shape Up | "Como terminamos proyectos en lugar de arrastrarlos" |
| SAFe | "Como coordinamos 50+ equipos" |
| DDD | "Como dividimos un sistema complejo en partes claras" |
| TDD | "Como aseguramos que el codigo funciona antes de ship" |
| BDD | "Como alineamos tests con lenguaje de negocio" |
| CI/CD | "Como integramos cambios sin caos" |
| DORA | "Como medimos y mejoramos performance de entrega" |
| Continuous Discovery | "Como sabemos que construir antes de commitment" |
| Spec-Driven | "Como hacemos que la AI construya lo que queremos" |

### Que nivel organizacional ataca

| Framework | Equipo | Producto | Portfolio | Empresa |
|---|---|---|---|---|
| Scrum | ✅ | ❌ | ❌ | ❌ |
| Kanban | ✅ | ✅ (flow) | ❌ | ❌ |
| XP | ✅ | ❌ | ❌ | ❌ |
| Shape Up | ✅ | ✅ | ❌ | ❌ |
| SAFe | ✅ | ✅ | ✅ | ✅ |
| DDD | ✅ | ✅ | ✅ | ✅ (estrategico) |
| TDD/BDD | ✅ | ❌ | ❌ | ❌ |
| CI/CD | ✅ | ✅ | ❌ | ❌ |
| DORA | ✅ | ✅ | ✅ | ✅ |
| Continuous Discovery | ✅ | ✅ | ❌ | ❌ |
| Spec-Driven | ✅ | ✅ | ❌ | ❌ |

### Madurez y adopcion (2026)

| Framework | Adopcion | Madurez | Tendencia |
|---|---|---|---|
| Scrum | Muy alta (+60% equipos Agile) | Maduro | Estable |
| Kanban | Muy alta | Maduro, guia 2025 fresca | Creciendo |
| XP | Baja (puro) | Maduro | Resurgimiento (era AI) |
| Shape Up | Media-nicha | Maduro | Creciendo |
| SAFe | Alta en enterprise (~30% Fortune 100) | Maduro, v6.0 + AI-Native | Estable |
| DDD | Media-alta en sistemas complejos | Maduro | Creciendo (con microservicios) |
| TDD | Media | Maduro, redefinido 2023 | Resurgimiento (era AI) |
| BDD | Media-baja | Maduro | Estable |
| CI/CD | Muy alta | Maduro, Fowler 2024 update | Estable |
| DORA | Muy alta (referencia) | Actualizado anual | Creciendo |
| Continuous Discovery | Media | Emergente (libro 2021) | Creciendo |
| Spec-Driven | Emergente | Alpha / nuevo | Creciendo rapido |

### Combinaciones comunes en 2026

1. **Scrum + TDD + CI/CD + DORA metrics** — el default de la mayoria de empresas
2. **Shape Up + DDD + Spec-Driven** — equipos chicos, alta autonomia, productos nuevos
3. **Kanban + DORA + Continuous Discovery** — ops, infra, plataformas internas
4. **SAFe + DDD + TDD + CI/CD** — enterprise, multi-equipo
5. **XP + TDD + BDD + CI/CD + Spec-Driven** — equipos de producto con cultura engineering fuerte

---

## Notas para el contexto Sixbell

- Sixbell usa una variante de **OpenSpec** (que es Spec-Driven) sobre **Scrum** y **DORA metrics** en sus 3 productos.
- El steering de VOC Analytics, GenIA, Insights referencia continuamente la metodologica `six_iagen_methodology` — que combina proposal + design + specs por repo, similar a OpenSpec + Kiro.
- El usuario (Andres) tiene preferencia explicita por **specs formales + tests** sobre improvisacion (regla scope_rule: "TEST/CI-CD/SEGURIDAD/DOCS > cualquier mejora funcional").

## Referencias oficiales

- Manifiesto Agile: https://agilemanifesto.org
- Scrum Guide 2020: https://scrumguides.org/scrum-guide.html
- Kanban Guide 2025.5: https://kanbanguides.org/the-kanban-guide/
- Shape Up: https://basecamp.com/shapeup
- Continuous Integration (Fowler, 2024): https://martinfowler.com/articles/continuousIntegration.html
- TDD (Fowler, 2023): https://martinfowler.com/bliki/TestDrivenDevelopment.html
- DORA: https://dora.dev/research/
- SAFe: https://www.scaledagileframework.com/
- Lean: https://www.lean.org/explore-lean/what-is-lean/
- DDD: https://www.domainlanguage.com/ddd/
- Continuous Discovery: https://www.producttalk.org/continuous-discovery-habits/
