package io.github.alexperez7.snesport;

import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageInstaller;
import android.widget.Toast;

/** El resultado del instalador de Android: enseña su confirmación, o dice por qué falló. */
public class UpdateReceiver extends BroadcastReceiver {
    @Override
    @SuppressWarnings("deprecation")
    public void onReceive(Context c, Intent i) {
        int status = i.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE);
        if (status == PackageInstaller.STATUS_PENDING_USER_ACTION) {
            Intent confirm = i.getParcelableExtra(Intent.EXTRA_INTENT);
            if (confirm != null) {
                confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
                c.startActivity(confirm);
            }
        } else if (status != PackageInstaller.STATUS_SUCCESS) {
            // Un usuario que cancela no es un error que merezca un mensaje.
            if (status != PackageInstaller.STATUS_FAILURE_ABORTED)
                Toast.makeText(c, c.getString(R.string.upd_fail_install, String.valueOf(i.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE))), Toast.LENGTH_LONG).show();
        }
    }
}
