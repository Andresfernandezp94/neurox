package net.neurox.app

import android.content.Context
import android.content.SharedPreferences

/**
 * Guarda la direccion del servidor de Neurox.
 *
 * Se guarda "host:port" y no la URL entera para no arrastrar el esquema:
 * neurox-web sirve por HTTP y el esquema decide elNetwork Security Config.
 */
class ServerPrefs(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(FILE, Context.MODE_PRIVATE)

    /** Host configurado, o null si la app aun no se ha configurado. */
    fun host(): String? = prefs.getString(KEY_HOST, null)

    fun port(): Int = prefs.getInt(KEY_PORT, DEFAULT_PORT)

    fun save(host: String, port: Int) {
        prefs.edit()
            .putString(KEY_HOST, host)
            .putInt(KEY_PORT, port)
            .apply()
    }

    /** forget: permite volver a la pantalla de ajustes desde la propia app. */
    fun clear() {
        prefs.edit().clear().apply()
    }

    companion object {
        private const val FILE = "neurox_prefs"
        private const val KEY_HOST = "host"
        private const val KEY_PORT = "port"
        const val DEFAULT_PORT = 8787

        /**
         * Normaliza lo que el usuario escribe a "host:port".
         *
         * Acepta las formas que se teclean de verdad y evita que la app
         * quede apuntando a algo invalido:
         *   "192.168.1.5", "http://192.168.1.5:8787", "neurox.local/"
         *   -> "192.168.1.5" / 8787
         *
         * Devuelve null si no queda un host usable.
         */
        fun normalize(input: String): Pair<String, Int>? {
            var s = input.trim()
            if (s.isEmpty()) return null

            // Quita el esquema
            s = s.substringAfter("://", s)
            // Corta ruta, query y fragmento
            s = s.substringBefore('/').substringBefore('?').substringBefore('#')
            if (s.isEmpty()) return null

            var host = s
            var port = DEFAULT_PORT

            // Puerto explicito: tras el ultimo ':' que no sea IPv6
            val colon = s.lastIndexOf(':')
            if (colon > 0 && colon < s.length - 1 && !s.endsWith(":")) {
                val maybePort = s.substring(colon + 1)
                val parsed = maybePort.toIntOrNull()
                if (parsed != null && parsed in 1..65535) {
                    host = s.substring(0, colon)
                    port = parsed
                }
            }

            // IPv6 entre corchetes: [::1]:8787
            if (host.startsWith("[") && host.endsWith("]")) {
                host = host.substring(1, host.length - 1)
            }
            if (host.isEmpty()) return null

            return host to port
        }

        /** URL completa que se carga en el WebView. */
        fun urlFor(host: String, port: Int): String = "http://$host:$port"
    }
}