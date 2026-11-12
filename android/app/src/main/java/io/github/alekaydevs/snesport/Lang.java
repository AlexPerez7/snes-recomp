package io.github.alekaydevs.snesport;

import android.content.Context;
import android.content.SharedPreferences;
import android.content.res.Configuration;
import java.util.Locale;

/** El idioma de la app: el del teléfono (inglés o español) hasta que el jugador lo cambia con el botón del globo. */
final class Lang {
    private Lang() {}

    private static SharedPreferences prefs(Context c) {
        return c.getSharedPreferences("lang", Context.MODE_PRIVATE);
    }

    /** «en» o «es»: lo elegido, o si no, el del teléfono (español solo si el teléfono está en español). */
    static String get(Context c) {
        String v = prefs(c).getString("lang", "");
        if (v.equals("en") || v.equals("es")) return v;
        return Locale.getDefault().getLanguage().equals("es") ? "es" : "en";
    }

    static void toggle(Context c) {
        prefs(c).edit().putString("lang", get(c).equals("es") ? "en" : "es").apply();
    }

    /** Para `attachBaseContext`: los recursos salen en el idioma elegido. */
    static Context wrap(Context base) {
        Configuration cfg = new Configuration(base.getResources().getConfiguration());
        cfg.setLocale(new Locale(get(base)));
        return base.createConfigurationContext(cfg);
    }
}
