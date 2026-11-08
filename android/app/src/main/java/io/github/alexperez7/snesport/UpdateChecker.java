package io.github.alexperez7.snesport;

import android.content.Context;
import android.content.SharedPreferences;
import android.content.pm.ApplicationInfo;
import java.io.File;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import org.json.JSONObject;

/**
 * Aviso opcional de versión nueva (cambio android-updates): consulta la última release de GitHub y compara su
 * etiqueta con la versión instalada. Si la release trae un APK y su SHA256SUMS, la app puede descargarlo e instalarlo
 * (UpdateInstaller); si no, el aviso lleva a la página de la release. Activo por defecto (se apaga en los ajustes).
 */
final class UpdateChecker {
    static final long DAY_MS = 24L * 60 * 60 * 1000;
    private static final int TIMEOUT_MS = 2500;

    /** Lo que se sabe de la última release. `ok` es false si no se pudo consultar; `newer` si es mayor que la instalada. */
    static final class Result {
        final boolean ok, newer;
        final String tag, url;
        /** El APK de la release (descarga directa), su nombre y el SHA256SUMS de la misma release; vacíos si no los hay. */
        final String apkUrl, apkName, sumsUrl;

        Result(boolean ok, boolean newer, String tag, String url) {
            this(ok, newer, tag, url, "", "", "");
        }

        Result(boolean ok, boolean newer, String tag, String url, String apkUrl, String apkName, String sumsUrl) {
            this.ok = ok;
            this.newer = newer;
            this.tag = tag;
            this.url = url;
            this.apkUrl = apkUrl;
            this.apkName = apkName;
            this.sumsUrl = sumsUrl;
        }

        boolean canInstall() {
            return !apkUrl.isEmpty() && !apkName.isEmpty() && !sumsUrl.isEmpty();
        }
    }

    private UpdateChecker() {}

    private static SharedPreferences prefs(Context c) {
        return c.getSharedPreferences("updates", Context.MODE_PRIVATE);
    }

    /** La dirección de la API de la última release, o "" si esta compilación no tiene actualizaciones. */
    static String url(Context c) {
        if ((c.getApplicationInfo().flags & ApplicationInfo.FLAG_DEBUGGABLE) != 0) {
            // Solo en depuración: files/update_url sustituye la dirección, para probar sin GitHub.
            File f = new File(c.getFilesDir(), "update_url");
            if (f.isFile()) {
                try (InputStream in = new java.io.FileInputStream(f)) {
                    return new String(in.readAllBytes(), StandardCharsets.UTF_8).trim();
                } catch (java.io.IOException e) {
                    // se usa la de la compilación
                }
            }
        }
        return c.getString(R.string.update_url).trim();
    }

    static boolean available(Context c) {
        return !url(c).isEmpty();
    }

    static boolean enabled(Context c) {
        return available(c) && prefs(c).getBoolean("check", true);
    }

    static void setEnabled(Context c, boolean on) {
        prefs(c).edit().putBoolean("check", on).apply();
    }

    static String installedVersion(Context c) {
        try {
            String v = c.getPackageManager().getPackageInfo(c.getPackageName(), 0).versionName;
            return v == null ? "0" : v;
        } catch (Exception e) {
            return "0";
        }
    }

    /** Compara dos versiones por partes numéricas («v0.10» > «0.9»); lo que no es un número cuenta como 0. */
    static int compare(String a, String b) {
        String[] pa = parts(a), pb = parts(b);
        for (int i = 0; i < Math.max(pa.length, pb.length); i++) {
            long x = i < pa.length ? number(pa[i]) : 0, y = i < pb.length ? number(pb[i]) : 0;
            if (x != y) return x < y ? -1 : 1;
        }
        return 0;
    }

    private static String[] parts(String v) {
        v = v.trim();
        if (v.startsWith("v") || v.startsWith("V")) v = v.substring(1);
        return v.split("\\.");
    }

    private static long number(String s) {
        int n = 0;
        while (n < s.length() && Character.isDigit(s.charAt(n))) n++;
        if (n == 0 || n > 9) return 0;
        return Long.parseLong(s.substring(0, n));
    }

    /** Una etiqueta que se pueda leer como versión: empieza por un dígito (tras una «v» opcional). */
    static boolean looksLikeVersion(String tag) {
        String t = tag == null ? "" : tag.trim();
        if (t.startsWith("v") || t.startsWith("V")) t = t.substring(1);
        return !t.isEmpty() && Character.isDigit(t.charAt(0));
    }

    /** Consulta la red (bloquea: usar en un hilo) y guarda el resultado con la hora. */
    static Result query(Context c) {
        String url = url(c);
        if (url.isEmpty()) return new Result(false, false, "", "");
        HttpURLConnection conn = null;
        try {
            conn = (HttpURLConnection) new URL(url).openConnection();
            conn.setConnectTimeout(TIMEOUT_MS);
            conn.setReadTimeout(TIMEOUT_MS);
            conn.setRequestProperty("Accept", "application/vnd.github+json");
            conn.setRequestProperty("User-Agent", "alttp-recomp-android");
            if (conn.getResponseCode() != 200) return new Result(false, false, "", "");
            String body;
            try (InputStream in = conn.getInputStream()) {
                body = new String(in.readNBytes(1 << 20), StandardCharsets.UTF_8);
            }
            JSONObject json = new JSONObject(body);
            String tag = json.optString("tag_name", ""), page = json.optString("html_url", "");
            if (!looksLikeVersion(tag) || !page.startsWith("https://")) return new Result(false, false, "", "");
            String apk = "", apkName = "", sums = "";
            org.json.JSONArray assets = json.optJSONArray("assets");
            for (int i = 0; assets != null && i < assets.length(); i++) {
                JSONObject a = assets.optJSONObject(i);
                if (a == null) continue;
                String name = a.optString("name", ""), dl = a.optString("browser_download_url", "");
                if (!dl.startsWith("https://github.com/")) continue;
                if (name.matches("[A-Za-z0-9._-]+\\.apk")) { apk = dl; apkName = name; }
                else if (name.equals("SHA256SUMS")) sums = dl;
            }
            prefs(c).edit().putLong("last", System.currentTimeMillis()).putString("tag", tag).putString("link", page)
                .putString("apk", apk).putString("apkname", apkName).putString("sums", sums).apply();
            return fromCache(c);
        } catch (Exception e) {
            return new Result(false, false, "", "");
        } finally {
            if (conn != null) conn.disconnect();
        }
    }

    /** Lo último que se supo, comparado con la versión instalada ahora; `ok` es false si nunca se consultó. */
    static Result fromCache(Context c) {
        SharedPreferences p = prefs(c);
        String tag = p.getString("tag", ""), link = p.getString("link", "");
        if (tag.isEmpty() || link.isEmpty()) return new Result(false, false, "", "");
        return new Result(true, compare(tag, installedVersion(c)) > 0, tag, link,
            p.getString("apk", ""), p.getString("apkname", ""), p.getString("sums", ""));
    }

    /** ¿Hace menos de 24 horas de la última consulta? */
    static boolean fresh(Context c) {
        return System.currentTimeMillis() - prefs(c).getLong("last", 0) < DAY_MS;
    }

    static boolean skipped(Context c, String tag) {
        return tag.equals(prefs(c).getString("skipped", ""));
    }

    static void skip(Context c, String tag) {
        prefs(c).edit().putString("skipped", tag).apply();
    }
}
