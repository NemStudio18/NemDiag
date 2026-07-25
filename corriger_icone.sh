#!/bin/bash

echo "=== Correction de l'icône de NemDiag ==="

# Vérification des droits administrateur
if [ "$EUID" -ne 0 ]; then 
  echo "Erreur : Veuillez lancer ce script avec sudo (ex: sudo bash corriger_icone.sh)"
  exit 1
fi

# Création du dossier d'icône si inexistant
mkdir -p /usr/share/icons/hicolor/128x128/apps/

# Copie du logo
if [ -f "icons/128x128.png" ]; then
    cp icons/128x128.png /usr/share/icons/hicolor/128x128/apps/nemdiag.png
    echo "Logo copié dans le système."
else
    echo "Attention: L'image icons/128x128.png est introuvable."
    exit 1
fi

# Modification du fichier .desktop
DESKTOP_FILE="/usr/share/applications/nemdiag.desktop"
if [ -f "$DESKTOP_FILE" ]; then
    # On remplace l'icône générique par notre icône personnalisée
    sed -i 's/^Icon=.*/Icon=nemdiag/' "$DESKTOP_FILE"
    echo "Fichier .desktop mis à jour."
else
    echo "Attention: Le fichier $DESKTOP_FILE n'existe pas."
    exit 1
fi

# Rafraîchissement du cache des icônes
update-icon-caches /usr/share/icons/hicolor
update-desktop-database /usr/share/applications/

echo "=== Terminé ! L'icône devrait maintenant apparaître dans votre menu Démarrer. ==="
echo "Note : Si elle n'apparaît pas immédiatement, déconnectez-vous et reconnectez-vous à votre session."
