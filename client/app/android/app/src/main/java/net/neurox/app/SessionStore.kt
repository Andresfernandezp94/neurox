package net.neurox.app

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * Guarda la sesión fuera del WebView, cifrada.
 *
 * El panel web guarda su JWT en `sessionStorage`, que Android destruye junto
 * al proceso: al reabrir la app había que autenticarse otra vez. Aquí se
 * guarda el **refresh token** en `EncryptedSharedPreferences`, cuya clave
 * maestra vive en el Keystore del sistema.
 *
 * Por qué no se persiste también el JWT de acceso: dura 24 h y el refresh
 * 30 días. Si el dispositivo se pierde, interesa exponer lo menos posible.
 * Con solo el refresh token, el robo da una credencial revocable desde la
 * web y que caduca sola; el JWT se renueva en memoria.
 *
 * El trade-off asumido: si el daemon pierde el store de refresh (se borra el
 * fichero o se restaura una copia antigua), el móvil pierde la sesión
 * aunque el navegador siga dentro de las 24 h del JWT. A cambio, es
 * imposible que un backup antiguo del dispositivo reabra una sesión
 * revocada.
 */
class SessionStore(context: Context) {

    private val prefs: SharedPreferences? = runCatching {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        EncryptedSharedPreferences.create(
            context,
            FILE,
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
        )
    }.getOrElse { e ->
        // Un Keystore corrupto (cambio de pantalla de bloqueo, restore de
        // backup) no debe impedir abrir la app: se sigue sin persistencia.
        Log.w(TAG, "almacén cifrado no disponible, la sesión no persistirá", e)
        null
    }

    fun refreshToken(): String? = prefs?.getString(KEY_REFRESH, null)

    fun saveRefreshToken(token: String) {
        prefs?.edit()?.putString(KEY_REFRESH, token)?.apply()
    }

    /** Host al que pertenece la sesión, para no enviarla a otro servidor. */
    fun sessionHost(): String? = prefs?.getString(KEY_HOST, null)

    fun saveSession(host: String) {
        prefs?.edit()?.putString(KEY_HOST, host)?.apply()
    }

    fun clear() {
        prefs?.edit()?.clear()?.apply()
    }

    /**
     * Solo si la sesión guardada es del host indicado. Cambiar de servidor
     * en los ajustes debe invalidar la sesión, no dejarla huérfana.
     */
    fun refreshTokenFor(host: String): String? =
        if (sessionHost() == host) refreshToken() else null

    private companion object {
        const val TAG = "SessionStore"
        const val FILE = "neurox_session"
        const val KEY_REFRESH = "refresh_token"
        const val KEY_HOST = "session_host"
    }
}