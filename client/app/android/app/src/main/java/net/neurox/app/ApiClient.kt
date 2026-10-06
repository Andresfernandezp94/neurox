package net.neurox.app

import org.json.JSONObject
import java.io.BufferedReader
import java.net.HttpURLConnection
import java.net.URL

/**
 * Cliente HTTP mínimo contra el panel.
 *
 * No usa OkHttp ni Retrofit a propósito: son dos peticiones (login y canje de
 * refresh) y añadir una librería de red solo por esto serían ~1 MB de APK.
 * Lo que sí hace falta es NO ir por el WebView para esto, porque el token
 * vive en la sesión del WebView, que es justo lo que se destruye al cerrar
 * la app.
 */
class ApiClient {

    /**
     * Canjea el refresh token por un JWT. Devuelve también el refresh token
     * rotado, que el daemon emite nuevo en cada llamada.
     */
    fun refresh(baseUrl: String, refreshToken: String): Result<RefreshResult> =
        runCatching {
            val body = JSONObject()
                .put("refresh_token", refreshToken)
                .put("client", "android")
                .toString()

            val res = post("$baseUrl/v1/auth/refresh-token", body)
            if (res.status !in 200..299) {
                // 401 aquí significa "token inválido o caducado": la sesión
                // se limpia y el usuario vuelve al login. Es el caso normal
                // tras 30 días, no un error que hidingar.
                throw ApiException(res.status, res.body)
            }

            val json = JSONObject(res.body)
            val token = json.optString("token").takeIf { it.isNotBlank() }
                ?: throw ApiException(res.status, "respuesta sin token")
            val rotated = json.optString("refresh_token").takeIf { it.isNotBlank() }
            RefreshResult(token = token, rotatedRefreshToken = rotated)
        }

    /** Login con usuario y contraseña. Mismo contrato que la web. */
    fun login(baseUrl: String, username: String, password: String): Result<RefreshResult> =
        runCatching {
            val body = JSONObject()
                .put("username", username)
                .put("password", password)
                .toString()

            val res = post("$baseUrl/v1/auth/login", body)
            if (res.status !in 200..299) {
                throw ApiException(res.status, res.body)
            }
            val json = JSONObject(res.body)
            val token = json.optString("token").takeIf { it.isNotBlank() }
                ?: throw ApiException(res.status, "respuesta sin token")
            RefreshResult(
                token = token,
                rotatedRefreshToken = json.optString("refresh_token")
                    .takeIf { it.isNotBlank() },
            )
        }

    /** True si el daemon tiene Google Sign-In configurado. */
    fun googleConfigured(baseUrl: String): Boolean = runCatching {
        val conn = URL("$baseUrl/health").openConnection() as HttpURLConnection
        conn.connectTimeout = 3000
        conn.readTimeout = 3000
        conn.requestMethod = "GET"
        conn.inputStream.bufferedReader().use { it.readText() }.contains("\"google\":true")
    }.getOrDefault(false)

    private fun post(url: String, body: String): HttpResponse {
        val conn = URL(url).openConnection() as HttpURLConnection
        return try {
            conn.requestMethod = "POST"
            conn.connectTimeout = 8000
            conn.readTimeout = 8000
            conn.doOutput = true
            conn.setRequestProperty("Content-Type", "application/json")
            conn.outputStream.use { it.write(body.toByteArray(Charsets.UTF_8)) }
            val code = conn.responseCode
            // El daemon responde 401 con cuerpo JSON, no en errorStream vacío.
            val text = (if (code in 200..299) conn.inputStream else conn.errorStream)
                ?.bufferedReader()?.use(BufferedReader::readText).orEmpty()
            HttpResponse(code, text)
        } finally {
            conn.disconnect()
        }
    }

    private data class HttpResponse(val status: Int, val body: String)

    data class RefreshResult(
        val token: String,
        val rotatedRefreshToken: String?,
    )

    class ApiException(val status: Int, message: String) :
        Exception(message.ifBlank { "HTTP $status" })
}