package net.neurox.app

import android.graphics.Color
import android.os.Bundle
import android.view.ViewGroup
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.compose.ui.platform.ComposeView
import com.getcapacitor.BridgeActivity
import java.net.HttpURLConnection
import java.net.URL

/**
 * Envoltura del panel web de Neurox.
 *
 * El panel lo sirve `neurox-web`, que hace de proxy de las rutas /v1/ hacia el daemon.
 * Cargarlo en un WebView significa que la app hereda el mismo origen, sin CORS
 * y con el JWT en un solo salto, exactamente igual que en un navegador.
 *
 * El daemon no se expone: sigue escuchando solo en 127.0.0.1.
 */
class MainActivity : BridgeActivity() {

    private lateinit var prefs: ServerPrefs

    private var configView: ComposeView? = null
    private var checking = false
    private var configError: String? = null

    /** Evita que el load inicial de Capacitor (https://localhost) se confunda con un fallo real. */
    private var loadingRemote = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        prefs = ServerPrefs(this)
        watchForConnectionErrors()

        val host = prefs.host()
        if (host.isNullOrBlank()) {
            showConfig()
        } else {
            loadPanel(host, prefs.port())
        }
    }

    private fun loadPanel(host: String, port: Int) {
        loadingRemote = true
        bridge.webView.loadUrl(ServerPrefs.urlFor(host, port))
    }

    /**
     * Si el panel no carga (servidor apagado, IP cambiada por DHCP) se vuelve
     * a la pantalla de ajustes en vez de dejar una pagina en blanco.
     */
    private fun watchForConnectionErrors() {
        bridge.webView.webViewClient = object : WebViewClient() {
            override fun onReceivedError(
                view: WebView?,
                request: WebResourceRequest?,
                error: WebResourceError?,
            ) {
                super.onReceivedError(view, request, error)
                if (!loadingRemote || request?.isForMainFrame != true) return
                loadingRemote = false
                showConfig(
                    "No se pudo cargar el panel. Comprueba que neurox-web siga en marcha " +
                        "y que la direccion sea correcta.",
                )
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                loadingRemote = false
            }
        }
    }

    // ---- Pantalla de configuracion ------------------------------------------

    /**
     * Se superpone con addContentView en vez de setContentView: el WebView del
     * bridge queda adjunta al arbol, asi que al ocultar la pantalla sigue
     * vivo y no hay que recrearlo.
     */
    private fun showConfig(error: String? = null) {
        configError = error
        checking = false

        if (configView == null) {
            val view = ComposeView(this)
            addContentView(
                view,
                ViewGroup.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT,
                    ViewGroup.LayoutParams.MATCH_PARENT,
                ),
            )
            configView = view
        }
        renderConfig()
    }

    private fun renderConfig() {
        configView?.setContent {
            ServerConfigScreen(
                initialHost = prefs.host(),
                checking = checking,
                error = configError,
                onSubmit = { submit(it) },
            )
        }
    }

    private fun hideConfig() {
        configView?.let { (it.parent as? ViewGroup)?.removeView(it) }
        configView = null
        configError = null
    }

    /**
     * Valida el servidor antes de guardarlo, para no dejar la app apuntando a
     * algo inalcanzable. 401 y 403 cuentan como "el servidor esta ahi".
     */
    private fun submit(input: String) {
        val parsed = ServerPrefs.normalize(input)
        if (parsed == null) {
            configError = "No se reconoce esa direccion."
            renderConfig()
            return
        }

        val (host, port) = parsed
        val url = ServerPrefs.urlFor(host, port)

        checking = true
        configError = null
        renderConfig()

        Thread {
            val reachable = try {
                val conn = URL(url).openConnection() as HttpURLConnection
                conn.connectTimeout = 4000
                conn.readTimeout = 4000
                conn.requestMethod = "GET"
                conn.connect()
                val code = conn.responseCode
                conn.disconnect()
                code in 200..499
            } catch (_: Exception) {
                false
            }

            runOnUiThread {
                checking = false
                if (reachable) {
                    prefs.save(host, port)
                    hideConfig()
                    loadPanel(host, port)
                } else {
                    configError = "No responde en $host:$port. Comprueba que neurox-web " +
                        "esté en marcha y que la IP sea la actual."
                    renderConfig()
                }
            }
        }.start()
    }
}