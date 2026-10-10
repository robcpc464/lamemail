#pragma once

#include <QObject>
#include <QPointer>
#include <QQuickTextDocument>
#include <QTextBlock>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>
#include <optional>

class Folding : public QObject {
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QQuickTextDocument* document READ document WRITE setDocument NOTIFY documentChanged)
    Q_PROPERTY(QVariantList folds READ folds NOTIFY foldsChanged)

public:
    using QObject::QObject;

    QQuickTextDocument* document() const { return m_document; }
    void setDocument(QQuickTextDocument* document);
    QVariantList folds() const { return m_folds; }

    Q_INVOKABLE void toggle(int position);
    Q_INVOKABLE void fold(int position);
    Q_INVOKABLE void foldAll();
    Q_INVOKABLE void reveal(int position);
    Q_INVOKABLE void replace(const QString& replacement);

signals:
    void documentChanged();
    void foldsChanged();

private:
    struct Fold {
        QTextBlock header;
        QTextBlock first;
        QTextBlock last;
    };

    QTextDocument* text() const;
    QList<Fold> find() const;
    std::optional<Fold> at(int position) const;
    void setFolded(const Fold& fold, bool folded);
    void refresh();

    QPointer<QQuickTextDocument> m_document;
    QVariantList m_folds;
};
