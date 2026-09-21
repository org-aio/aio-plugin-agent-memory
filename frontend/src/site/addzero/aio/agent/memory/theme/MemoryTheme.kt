package site.addzero.aio.agent.memory.theme

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private fun typography(font: FontFamily?): Typography {
    val base = Typography()
    return Typography(
        displayLarge = base.displayLarge.copy(fontFamily = font, letterSpacing = 0.sp), displayMedium = base.displayMedium.copy(fontFamily = font, letterSpacing = 0.sp), displaySmall = base.displaySmall.copy(fontFamily = font, letterSpacing = 0.sp),
        headlineLarge = base.headlineLarge.copy(fontFamily = font, letterSpacing = 0.sp), headlineMedium = base.headlineMedium.copy(fontFamily = font, letterSpacing = 0.sp), headlineSmall = base.headlineSmall.copy(fontFamily = font, letterSpacing = 0.sp),
        titleLarge = base.titleLarge.copy(fontFamily = font, letterSpacing = 0.sp), titleMedium = base.titleMedium.copy(fontFamily = font, letterSpacing = 0.sp), titleSmall = base.titleSmall.copy(fontFamily = font, letterSpacing = 0.sp),
        bodyLarge = base.bodyLarge.copy(fontFamily = font, letterSpacing = 0.sp), bodyMedium = base.bodyMedium.copy(fontFamily = font, letterSpacing = 0.sp), bodySmall = base.bodySmall.copy(fontFamily = font, letterSpacing = 0.sp),
        labelLarge = base.labelLarge.copy(fontFamily = font, letterSpacing = 0.sp), labelMedium = base.labelMedium.copy(fontFamily = font, letterSpacing = 0.sp), labelSmall = base.labelSmall.copy(fontFamily = font, letterSpacing = 0.sp),
    )
}

@Composable
internal fun MemoryTheme(content: @Composable () -> Unit) {
    // 字体是可选增强：界面先用系统字体立即渲染，字体就绪后再切换。
    // 不能把 content 挡在字体加载之后，否则字体失败或缓慢时会整页白屏。
    val fonts = rememberMemoryFont()
    MaterialTheme(colorScheme = lightColorScheme(
        primary = Color(0xFF236F61), secondary = Color(0xFF48699B), tertiary = Color(0xFF984873),
        surface = Color(0xFFFCFDFD), background = Color(0xFFFCFDFD), onSurface = Color(0xFF252B2D),
        surfaceContainer = Color(0xFFF0F3F4), secondaryContainer = Color(0xFFE7EEF8), outlineVariant = Color(0xFFDDE3E5),
        surfaceContainerHigh = Color(0xFFF1F4F5), surfaceContainerHighest = Color(0xFFE6EBEC),
        surfaceContainerLow = Color(0xFFF6F8F9), surfaceContainerLowest = Color.White,
    ), typography = remember(fonts.font) { typography(fonts.font) },
        shapes = Shapes(small = RoundedCornerShape(4.dp), medium = RoundedCornerShape(8.dp), large = RoundedCornerShape(8.dp), extraLarge = RoundedCornerShape(8.dp))) {
        Surface(Modifier.fillMaxSize()) { content() }
    }
}
