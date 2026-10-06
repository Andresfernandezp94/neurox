package net.neurox.app

import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp

/**
 * Pantalla de carga mientras el panel se descarga.
 *
 * Antes el WebView mostraba blanco o negro durante el handshake con
 * neurox-web, sin ninguna señal de que estuviera pasando algo. Esto cubre
 * ese hueco y mantiene la coherencia visual con el login.
 */
@Composable
fun LoadingScreen(message: String) {
    MaterialTheme(colorScheme = NeuroxDark) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.background),
            contentAlignment = Alignment.Center,
        ) {
            Column(
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center,
            ) {
                SpinnerMark(size = 76.dp)
                Spacer(Modifier.height(22.dp))
                Text(
                    text = "neurox",
                    style = MaterialTheme.typography.headlineSmall,
                    color = MaterialTheme.colorScheme.primary,
                )
                Spacer(Modifier.height(6.dp))
                Text(
                    text = message,
                    style = MaterialTheme.typography.bodySmall,
                    color = Color(0xFF63636E),
                )
            }
        }
    }
}

/**
 * Anillo que gira, dibujado con Canvas en vez de tirar de `CircularProgressIndicator`
 * para poder igualar la marca del panel (trazo redondeado y hueco fino).
 */
@Composable
fun SpinnerMark(
    size: androidx.compose.ui.unit.Dp,
    color: Color = Color(0xFF9B8CFF),
    trackColor: Color = Color(0xFF26262E),
    strokeWidth: androidx.compose.ui.unit.Dp = 3.dp,
) {
    val transition = rememberInfiniteTransition(label = "spinner")
    val angle by transition.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1100, easing = LinearEasing),
            repeatMode = RepeatMode.Restart,
        ),
        label = "angle",
    )
    // Un halo que respira, para que el estado se note incluso sin movimiento.
    val glow by transition.animateFloat(
        initialValue = 0.35f,
        targetValue = 0.9f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1400, easing = LinearEasing),
            repeatMode = RepeatMode.Reverse,
        ),
        label = "glow",
    )

    Box(modifier = Modifier.size(size), contentAlignment = Alignment.Center) {
        Canvas(modifier = Modifier.fillMaxSize().rotate(angle)) {
            val strokePx = strokeWidth.toPx()
            val stroke = Stroke(width = strokePx, cap = StrokeCap.Round)
            val inset = strokePx / 2
            // `size` aquí es el del Canvas, no el Dp del parámetro.
            val box = Size(this.size.width - strokePx, this.size.height - strokePx)
            drawArc(
                color = trackColor,
                startAngle = 0f,
                sweepAngle = 360f,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = box,
                style = stroke,
            )
            // Un solo arco: un anillo completo parecería un borde fijo.
            drawArc(
                color = color.copy(alpha = glow),
                startAngle = 0f,
                sweepAngle = 96f,
                useCenter = false,
                topLeft = Offset(inset, inset),
                size = box,
                style = stroke,
            )
        }
    }
}