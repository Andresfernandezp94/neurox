package net.neurox.app

import android.graphics.Color
import android.os.Bundle
import android.util.Log
import android.view.ViewGroup
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.viewinterop.AndroidView
import com.getcapacitor.BridgeActivity
import org.json.JSONObject
import java.net.URL
import java.net.URLEncoder

/**
 * Arranque en frío del panel.
 *
 * El flujo tiene tres destinos, y el primero que aplica decide el resto:
 *
 *  1. **Sin servidor** → pantalla de ajustes (con validación en vivo).
 *  2. **Servidor, sin sesión** → login nativo contra `/v1/auth/login`.
 *  3. **Servidor, con refresh token** → canje por un JWT, se inyecta en la
 *     sesión del WebView y se entra directo al panel. Sin pedir la clave.
 *
 * El JWT no se persiste: vive solo en el `sessionStorage` del WebView. El
 * refresh token sí, cifrado en el Keystore (ver `SessionStore`). Así el
 * dispositivo guarda la credencial larga y revocable, no el token de acceso.
 */
class MainActivity : BridgeActivity() {

    private lateinit var prefs: ServerPrefs
    private lateinit var session: SessionStore
    private val api = ApiClient()

    private var overlay: ComposeView? = null
    private var checking = false
    private var overlayError: String? = null

    /** Evita confundir el `https://localhost` inicial de Capacitor con un fallo. */
    private var awaitingInjection = false
    private var pendingJwt: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        prefs = ServerPrefs(this)
        session = SessionStore(this)
        watchForConnectionErrors()

        val host = prefs.host()
        if (host.isNullOrBlank()) {
            showConfig()
        } else {
            restoreSession(host, prefs.port())
        }
    }

    private fun baseUrl(host: String, port: Int) = ServerPrefs.urlFor(host, port)

    // ── Destino 3: sesión guardada ──────────────────────────────────────

    private fun restoreSession(host: String, port: Int) {
        val refresh = session.refreshTokenFor(host)
        if (refresh.isNullOrBlank()) {
            // Nunca hubo sesión en este servidor: login.
            showLogin(host, port)
            return
        }

        showLoading("Restaurando sesión")
        val url = baseUrl(host, port)

        Thread {
            val result = api.refresh(url, refresh)
            runOnUiThread {
                when {
                    result.isFailure -> {
                        // Token caducado o revocado. Es el caso esperado a los
                        // 30 días, no una anomalía: se limpia y se login.
                        Log.i(TAG, "refresh rechazado, se vuelve al login")
                        session.clear()
                        hideOverlay()
                        showLogin(host, port, "Tu sesión caducó. Vuelve a entrar.")
                    }
                    else -> {
                        val r = result.getOrThrow()
                        // El daemon rota el token en cada canje: hay que
                        // guardar el nuevo o el siguiente arranque falla.
                        r.rotatedRefreshToken?.let { session.saveRefreshToken(it) }
                        injectTokenAndOpenPanel(host, port, r.token)
                    }
                }
            }
        }.start()
    }

    /**
     * El panel lee el JWT de `sessionStorage`, que es por origen. Se carga
     * primero el origen (una página cualquiera), se inyecta el token y solo
     * entonces se navega a `/app`. Si se hiciera al revés, la app arrancaría
     * sin sesión y redirigiría al login.
     */
    private fun injectTokenAndOpenPanel(host: String, port: Int, jwt: String) {
        showLoading("Abriendo neurox")
        pendingJwt = jwt
        awaitingInjection = true
        bridge.webView.loadUrl(baseUrl(host, port))
    }

    private fun onOriginReady() {
        val jwt = pendingJwt ?: return
        pendingJwt = null
        val host = prefs.host() ?: return
        val port = prefs.port()

        val encoded = URLEncoder.encode(jwt, "UTF-8")
        bridge.webView.evaluateJavascript(
            """
            try {
              sessionStorage.setItem('neurox_token', '$encoded');
              'ok';
            } catch (e) {
              'error: ' + e;
            }
            """.trimIndent(),
        ) { result ->
            runOnUiThread {
                if (result?.contains("ok") == true) {
                    Log.i(TAG, "token inyectado en la sesión del WebView")
                    awaitingInjection = false
                    bridge.webView.loadUrl("${baseUrl(host, port)}/app")
                } else {
                    Log.e(TAG, "no se pudo inyectar el token: $result")
                    awaitingInjection = false
                    hideOverlay()
                    showLogin(host, port, "No se pudo iniciar la sesión en el panel.")
                }
            }
        }
    }

    // ── Destino 2: login ────────────────────────────────────────────────

    private fun showLogin(host: String, port: Int, message: String? = null) {
        loginHost = host
        loginPort = port
        overlayError = null
        renderLogin(message)
    }

    private fun attemptLogin(host: String, port: Int, user: String, pass: String) {
        val url = baseUrl(host, port)
        checking = true
        overlayError = null
        renderOverlay()

        Thread {
            val result = api.login(url, user.trim(), pass)
            runOnUiThread {
                checking = false
                result
                    .onSuccess { r ->
                        r.rotatedRefreshToken?.let {
                            session.saveRefreshToken(it)
                            session.saveSession(host)
                        }
                        hideOverlay()
                        injectTokenAndOpenPanel(host, port, r.token)
                    }
                    .onFailure { e ->
                        // 401 = credenciales. Otro código = el servidor no
                        // responde, que es un problema distinto y conviene
                        // noMezclarlo con "contraseña incorrecta".
                        overlayError = when (e) {
                            is ApiClient.ApiException ->
                                if (e.status == 401) "Usuario o contraseña incorrectos."
                                else "El servidor respondió HTTP ${e.status}."
                            else -> "No se pudo contactar con $url."
                        }
                        renderOverlay()
                    }
            }
        }.start()
    }

    // ── Destino 1: ajustes de servidor ──────────────────────────────────

    private fun showConfig() {
        overlayError = null
        renderConfig()
    }

    private fun submitServer(input: String) {
        val parsed = ServerPrefs.normalize(input)
        if (parsed == null) {
            overlayError = "No se reconoce esa dirección."
            renderOverlay()
            return
        }
        val (host, port) = parsed
        val url = baseUrl(host, port)

        checking = true
        overlayError = null
        renderOverlay()

        Thread {
            val reachable = runCatching {
                val conn = URL(url).openConnection() as java.net.HttpURLConnection
                conn.connectTimeout = 4000
                conn.readTimeout = 4000
                conn.requestMethod = "GET"
                conn.connect()
                val code = conn.responseCode
                conn.disconnect()
                // 401 también confirma que hay algo escuchando.
                code in 200..499
            }.getOrDefault(false)

            runOnUiThread {
                checking = false
                if (reachable) {
                    prefs.save(host, port)
                    hideOverlay()
                    restoreSession(host, port)
                } else {
                    overlayError = "No responde en $host:$port. Comprueba que neurox-web " +
                        "esté en marcha y que la IP sea la actual."
                    renderOverlay()
                }
            }
        }.start()
    }

    // ── Overlay: ajustes / login / carga ───────────────────────────────

    private enum class OverlayKind { NONE, CONFIG, LOGIN, LOADING }

    private var overlayKind = OverlayKind.NONE
    private var loadingMessage = "Cargando"
    private var loginHost = ""
    private var loginPort = ServerPrefs.DEFAULT_PORT

    /**
     * El overlay se añade con `addContentView` y no con `setContentView`: el
     * WebView del bridge queda adjunto al árbol, así que al ocultarlo sigue
     * vivo y no hay que recrearlo.
     */
    private fun ensureOverlay(): ComposeView {
        overlay?.let { return it }
        val view = ComposeView(this)
        addContentView(
            view,
            ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT,
            ),
        )
        overlay = view
        return view
    }

    private fun renderConfig() {
        overlayKind = OverlayKind.CONFIG
        ensureOverlay().setContent {
            ServerConfigScreen(
                initialHost = prefs.host(),
                checking = checking,
                error = overlayError,
                onSubmit = { submitServer(it) },
            )
        }
    }

    private fun renderLogin(message: String? = null) {
        overlayKind = OverlayKind.LOGIN
        val host = loginHost
        val port = loginPort
        val err = overlayError ?: message
        ensureOverlay().setContent {
            LoginScreen(
                serverLabel = baseUrl(host, port),
                busy = checking,
                error = err,
                onSubmit = { u, p -> attemptLogin(host, port, u, p) },
                onChangeServer = {
                    session.clear()
                    prefs.clear()
                    showConfig()
                },
            )
        }
    }

    private fun renderLoading() {
        overlayKind = OverlayKind.LOADING
        val msg = loadingMessage
        ensureOverlay().setContent { LoadingScreen(msg) }
    }

    /** Re-dibuja la pantalla activa conservando su estado. */
    private fun renderOverlay() {
        when (overlayKind) {
            OverlayKind.CONFIG -> renderConfig()
            OverlayKind.LOGIN -> renderLogin()
            OverlayKind.LOADING -> renderLoading()
            OverlayKind.NONE -> Unit
        }
    }

    private fun showLoading(message: String) {
        loadingMessage = message
        overlayError = null
        renderLoading()
    }

    private fun hideOverlay() {
        overlay?.let { (it.parent as? ViewGroup)?.removeView(it) }
        overlay = null
        overlayKind = OverlayKind.NONE
        overlayError = null
    }

    // ── Errores de red del WebView ──────────────────────────────────────

    private fun watchForConnectionErrors() {
        bridge.webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                // El origen ya está cargado: es el momento de inyectar el
                // token, antes de que la SPA del panel lo busque.
                if (awaitingInjection) {
                    onOriginReady()
                } else if (overlayKind == OverlayKind.LOADING) {
                    hideOverlay()
                }
            }

            override fun onReceivedError(
                view: WebView?,
                request: WebResourceRequest?,
                error: WebResourceError?,
            ) {
                super.onReceivedError(view, request, error)
                if (request?.isForMainFrame != true) return
                val host = prefs.host() ?: return
                val port = prefs.port()
                awaitingInjection = false
                pendingJwt = null
                showLogin(
                    host,
                    port,
                    "No se pudo cargar el panel. Comprueba que neurox-web siga en marcha.",
                )
            }
        }
    }

    private companion object {
        const val TAG = "NeuroxApp"
    }
}