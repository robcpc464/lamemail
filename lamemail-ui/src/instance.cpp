#include "instance.h"

#include <QCryptographicHash>
#include <QDir>
#include <QLocalSocket>
#include <QStandardPaths>

namespace {

constexpr int waitMs = 1000;
const QString quitRequest = QStringLiteral("lamemail:quit");

QString socketPath(const QString& dir)
{
    const QByteArray key
        = QCryptographicHash::hash(QDir(dir).absolutePath().toUtf8(), QCryptographicHash::Sha1)
              .toHex()
              .left(16);
    return QStandardPaths::writableLocation(QStandardPaths::RuntimeLocation)
        + QStringLiteral("/lamemail-ui-") + QString::fromLatin1(key);
}

}

Instance::Instance(const QString& dir, QObject* parent)
    : QObject(parent)
    , m_name(socketPath(dir))
{
    connect(&m_server, &QLocalServer::newConnection, this, [this] {
        while (QLocalSocket* socket = m_server.nextPendingConnection()) {
            connect(socket, &QLocalSocket::disconnected, socket, &QObject::deleteLater);
            connect(socket, &QLocalSocket::readyRead, this, [this, socket] {
                if (socket->peek(socket->bytesAvailable()).count('\n') < 2)
                    return;
                const QByteArray token = socket->readLine().trimmed();
                const QString target = QString::fromUtf8(socket->readLine().chopped(1));
                socket->disconnectFromServer();
                if (target == quitRequest)
                    emit quitRequested();
                else
                    emit activated(token, target);
            });
        }
    });
}

bool Instance::handOver(const QByteArray& token, const QString& path)
{
    QLocalSocket socket;
    socket.connectToServer(m_name);
    if (!socket.waitForConnected(waitMs))
        return false;
    socket.write(token + '\n' + path.toUtf8() + '\n');
    return socket.waitForBytesWritten(waitMs);
}

bool Instance::requestQuit() { return handOver({ }, quitRequest); }

bool Instance::listen()
{
    QLocalServer::removeServer(m_name);
    return m_server.listen(m_name);
}
