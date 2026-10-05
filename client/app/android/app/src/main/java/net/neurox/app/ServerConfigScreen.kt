package net.neurox.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private val NeuroxDark = darkColorScheme(
    primary = Color(0xFF9B8CFF),
    onPrimary = Color(0xFF17123A),
    background = Color(0xFF101014),
    onBackground = Color(0xFFE6E6EA),
    surface = Color(0xFF17171C),
    onSurface = Color(0xFFE6E6EA),
    error = Color(0xFFFF6B6B),
)

@Composable
fun ServerConfigScreen(
    initialHost: String?,
    checking: Boolean,
    error: String?,
    onSubmit: (String) -> Unit,
) {
    var value by remember { mutableStateOf(initialHost ?: "") }

    MaterialTheme(colorScheme = NeuroxDark) {
        Surface(
            modifier = Modifier.fillMaxSize(),
            color = MaterialTheme.colorScheme.background,
        ) {
            Column(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(horizontal = 28.dp),
                verticalArrangement = Arrangement.Center,
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(
                    text = "Neurox",
                    style = MaterialTheme.typography.displaySmall,
                    fontFamily = FontFamily.SansSerif,
                    color = MaterialTheme.colorScheme.primary,
                )
                Spacer(Modifier.height(10.dp))
                Text(
                    text = "Direccion del servidor en tu red local",
                    style = MaterialTheme.typography.bodyMedium,
                    color = Color(0xFF9A9AA4),
                )

                Spacer(Modifier.height(36.dp))

                OutlinedTextField(
                    value = value,
                    onValueChange = { value = it },
                    singleLine = true,
                    enabled = !checking,
                    isError = error != null,
                    placeholder = { Text("192.168.1.5:${ServerPrefs.DEFAULT_PORT}") },
                    supportingText = {
                        Text(
                            text = error ?: "Solo host y puerto. El esquema y la ruta se ignoran.",
                            fontSize = 12.sp,
                        )
                    },
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Uri,
                        imeAction = ImeAction.Go,
                    ),
                    modifier = Modifier.fillMaxWidth(),
                )

                Spacer(Modifier.height(22.dp))

                Button(
                    onClick = { onSubmit(value) },
                    enabled = !checking && value.isNotBlank(),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(50.dp),
                ) {
                    if (checking) {
                        CircularProgressIndicator(
                            modifier = Modifier.height(18.dp),
                            strokeWidth = 2.dp,
                            color = MaterialTheme.colorScheme.onPrimary,
                        )
                    } else {
                        Text("Conectar", fontSize = 15.sp)
                    }
                }

                Spacer(Modifier.height(18.dp))
                Text(
                    text = "Por defecto el panel se sirve en el puerto ${ServerPrefs.DEFAULT_PORT}",
                    style = MaterialTheme.typography.bodySmall,
                    color = Color(0xFF63636E),
                )
            }
        }
    }
}