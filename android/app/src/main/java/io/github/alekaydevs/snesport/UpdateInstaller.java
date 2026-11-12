package io.github.alekaydevs.snesport;

import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Descarga e instala la versión nueva desde la app: baja el SHA256SUMS y el APK de la release, comprueba el hash y le
 * pasa el APK al instalador de Android (PackageInstaller), que enseña su propia confirmación. Android solo instala si la
 * firma es la misma que la de la app instalada, así que una release ajena no puede reemplazarla.
 */
final class UpdateInstaller {
    static final String ACTION = "io.github.alekaydevs.snesport.INSTALL_STATUS";
    private static final int TIMEOUT_MS = 15000;
    private static final long MAX_APK = 200L << 20;

    /** Un fallo con un recurso de texto para enseñarlo al usuario. */
    static final class Failure extends Exception {
        final int resId;

        Failure(int resId) {
            super("update failure " + resId);
            this.resId = resId;
        }
    }

    interface Progress {
        void percent(int p);
    }

    private UpdateInstaller() {}

    private static HttpURLConnection open(String url) throws IOException {
        if (!url.startsWith("https://")) throw new IOException("not https");
        HttpURLConnection conn = (HttpURLConnection) new URL(url).openConnection();
        conn.setConnectTimeout(TIMEOUT_MS);
        conn.setReadTimeout(TIMEOUT_MS);
        conn.setRequestProperty("User-Agent", "alttp-recomp-android");
        if (conn.getResponseCode() != 200) throw new IOException("HTTP " + conn.getResponseCode());
        return conn;
    }

    /** El hash esperado de `name` en el contenido de un SHA256SUMS («hash  nombre» o «hash *nombre»), o null. */
    static String expectedHash(String sums, String name) {
        for (String line : sums.split("\\r?\\n")) {
            String t = line.trim();
            int sp = t.indexOf(' ');
            if (sp != 64) continue;
            String file = t.substring(sp + 1).trim();
            if (file.startsWith("*")) file = file.substring(1);
            if (file.equals(name) && t.substring(0, 64).matches("[0-9A-Fa-f]{64}")) return t.substring(0, 64).toLowerCase();
        }
        return null;
    }

    /** Baja y verifica el APK (bloquea: usar en un hilo). Devuelve el fichero en la caché de la app. */
    static File download(Context c, UpdateChecker.Result r, Progress progress, AtomicBoolean cancel) throws Failure {
        String want;
        try {
            HttpURLConnection conn = open(r.sumsUrl);
            try (InputStream in = conn.getInputStream()) {
                want = expectedHash(new String(in.readNBytes(1 << 16), StandardCharsets.UTF_8), r.apkName);
            } finally {
                conn.disconnect();
            }
        } catch (IOException e) {
            throw new Failure(R.string.upd_fail_net);
        }
        if (want == null) throw new Failure(R.string.upd_fail_hash);
        File dest = new File(c.getCacheDir(), "update.apk");
        HttpURLConnection conn = null;
        try {
            conn = open(r.apkUrl);
            long total = conn.getContentLengthLong();
            if (total > MAX_APK) throw new Failure(R.string.upd_fail_net);
            MessageDigest md = MessageDigest.getInstance("SHA-256");
            long done = 0;
            try (InputStream in = conn.getInputStream(); OutputStream out = new FileOutputStream(dest)) {
                byte[] buf = new byte[1 << 16];
                for (int n; (n = in.read(buf)) > 0; ) {
                    if (cancel.get()) throw new Failure(R.string.upd_cancelled);
                    md.update(buf, 0, n);
                    out.write(buf, 0, n);
                    done += n;
                    if (done > MAX_APK) throw new Failure(R.string.upd_fail_net);
                    if (total > 0) progress.percent((int) (done * 100 / total));
                }
            }
            StringBuilder got = new StringBuilder();
            for (byte b : md.digest()) got.append(String.format("%02x", b));
            if (!got.toString().equals(want)) {
                dest.delete();
                throw new Failure(R.string.upd_fail_hash);
            }
            return dest;
        } catch (Failure f) {
            dest.delete();
            throw f;
        } catch (Exception e) {
            dest.delete();
            throw new Failure(R.string.upd_fail_net);
        } finally {
            if (conn != null) conn.disconnect();
        }
    }

    /** Entrega el APK ya verificado al instalador; el sistema pide confirmación y, al instalar, cierra la app. */
    static void install(Context c, File apk) throws IOException {
        PackageInstaller pi = c.getPackageManager().getPackageInstaller();
        PackageInstaller.SessionParams params = new PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL);
        int id = pi.createSession(params);
        try (PackageInstaller.Session s = pi.openSession(id)) {
            try (InputStream in = new FileInputStream(apk); OutputStream out = s.openWrite("update.apk", 0, apk.length())) {
                byte[] buf = new byte[1 << 16];
                for (int n; (n = in.read(buf)) > 0; ) out.write(buf, 0, n);
                s.fsync(out);
            }
            Intent intent = new Intent(c, UpdateReceiver.class).setAction(ACTION);
            PendingIntent pending = PendingIntent.getBroadcast(c, id, intent, PendingIntent.FLAG_MUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
            s.commit(pending.getIntentSender());
        }
        apk.delete();
    }
}
