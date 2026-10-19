// Private-bus Qt portal consumer. Never connects to the user's desktop portal.
#include <QtQuickTest/quicktest.h>
#include <QCoreApplication>
#include <QFile>
#include <QTemporaryDir>
#include <QUrl>
#include <QDBusConnection>
#include <QDBusArgument>
#include <QDBusObjectPath>
#include <QDBusVariant>
#include <QDBusMessage>
#include <QDBusVirtualObject>
#include <QTimer>
#include <QVariantMap>
#include <cstdio>

class Portal final : public QDBusVirtualObject {
public:
  int opens = 0;
  QTemporaryDir files;
  Portal() {
    for (const auto &name : {"first.txt", "second.txt", "profile.png"}) {
      QFile file(files.filePath(name));
      if (!file.open(QIODevice::WriteOnly)) qFatal("Fixture file creation failed");
      file.write("synthetic selection\n");
    }
  }
  QString introspect(const QString &) const override {
    return QStringLiteral(R"(<interface name="org.freedesktop.portal.FileChooser">
      <method name="OpenFile"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
      <property name="version" type="u" access="read"/>
    </interface><interface name="org.freedesktop.portal.Settings"><method name="Read"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="v" direction="out"/></method></interface>)");
  }
  bool handleMessage(const QDBusMessage &m, const QDBusConnection &bus) override {
    if (m.member() == "OpenFile") {
      opens++;
      const QString path = "/org/freedesktop/portal/desktop/request/fixture/" + QString::number(opens);
      bus.send(m.createReply(QVariant::fromValue(QDBusObjectPath(path))));
      const bool photo = m.arguments().value(1).toString().contains("profile");
      const bool cancel = m.arguments().value(1).toString().contains("Cancel fixture");
      const auto options = qdbus_cast<QVariantMap>(m.arguments().value(2));
      const bool multiple = options.value("multiple").toBool();
      std::fprintf(stderr, "PORTAL_OPEN multiple=%d photo=%d\n", multiple, photo);
      QTimer::singleShot(30, this, [this, bus, path, photo, multiple, cancel] {
        auto response = QDBusMessage::createSignal(path, "org.freedesktop.portal.Request", "Response");
        QStringList uris{QUrl::fromLocalFile(files.filePath(photo ? "profile.png" : "first.txt")).toString()};
        if (multiple) uris << QUrl::fromLocalFile(files.filePath("second.txt")).toString();
        response << uint(cancel ? 1 : 0) << (cancel ? QVariantMap{} : QVariantMap{{"uris", uris}});
        bus.send(response);
      });
      return true;
    }
    if (m.member() == "Get") {
      bus.send(m.createReply(QVariant::fromValue(QDBusVariant(uint(3)))));
      return true;
    }
    if (m.member() == "Read") {
      bus.send(m.createErrorReply("org.freedesktop.portal.Error.NotFound", "No fixture setting"));
      return true;
    }
    if (m.member() == "Close") { bus.send(m.createReply()); return true; }
    return false;
  }
};
int main(int argc, char **argv) {
  if (argc == 2 && QByteArray(argv[1]) == "--server") {
    QCoreApplication app(argc, argv);
    Portal portal;
    auto bus = QDBusConnection::sessionBus();
    if (!bus.registerService("org.freedesktop.portal.Desktop") ||
        !bus.registerVirtualObject("/org/freedesktop/portal/desktop", &portal, QDBusConnection::SubPath))
      qFatal("Private portal registration failed");
    std::puts("PORTAL_READY"); std::fflush(stdout);
    QTimer::singleShot(20000, &app, &QCoreApplication::quit);
    return app.exec();
  }
  if (qEnvironmentVariable("TELEBAR_FILE_PORTAL_TEST") != "1")
    qFatal("Run through tests/run-file-portal.sh on its private bus");
  return quick_test_main(argc, argv, "system_file_portal", nullptr);
}
