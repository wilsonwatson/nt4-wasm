package org.frc5572.nt4wasm;

import java.io.File;
import java.nio.file.Files;

import edu.wpi.first.wpilibj.Filesystem;

import io.javalin.Javalin;
import io.javalin.http.ContentType;
import io.javalin.http.HttpStatus;

public class WebDashboard {

    private static Javalin server;

    public static void start(boolean simulation) {
        server = Javalin.create(/* config */)
                .get("nt4.js", ctx -> {
                    ctx.contentType(ContentType.TEXT_JS)
                            .result(WebDashboard.class.getClassLoader().getResourceAsStream("nt4.js"));
                })
                .get("nt4_wasm_bg.wasm", ctx -> {
                    ctx.contentType(ContentType.APPLICATION_OCTET_STREAM)
                            .result(WebDashboard.class.getClassLoader().getResourceAsStream("nt4_wasm_bg.wasm"));
                })
                .get("/*", ctx -> {
                    String path = ctx.path();
                    if (path.endsWith("/")) {
                        path += "index.html";
                    }
                    String[] ext_ = path.split("\\.");
                    String ext = "TXT";
                    if (ext_.length > 0) {
                        ext = ext_[ext_.length - 1].toUpperCase();
                    }
                    File f = new File(new File(Filesystem.getDeployDirectory(), "dashboard"), path);
                    try {
                        byte[] data = Files.readAllBytes(f.toPath());
                        switch (ext) {
                            case "HTML":
                                ctx.contentType(ContentType.TEXT_HTML).result(data);
                                break;
                            case "JS":
                                ctx.contentType(ContentType.TEXT_JS)
                                        .result(data);
                                break;
                            case "WASM":
                                ctx.contentType(ContentType.APPLICATION_OCTET_STREAM).result(data);
                                break;
                            default:
                                ctx.contentType(ContentType.TEXT_PLAIN).result(data);
                                break;
                        }
                    } catch (Exception e) {
                        ctx.status(HttpStatus.NOT_FOUND);
                    }
                }).start(7070);

    }

    public static void stop() {
        server.stop();
    }

}