<p align="center">
	<img src="https://raw.githubusercontent.com/goldboot/goldboot/master/.github/images/logo-bg-256.png" />
</p>

`goldboot` simplifies building and deploying bare-metal golden images to server
or desktop environments.

Warning: this tool is totally unfinshed and should be used for testing only! Proceed
at your own risk!

### Getting Started

First, create a directory which can later be added to version control:
```sh
mkdir WindowsMachine
cd WindowsMachine
```

Initialize the directory and choose a base profile to start with:
```sh
goldboot init --profile Windows10
```

This will create `goldboot.json` which contains configuration options that will
need to be tweaked. For example, you'll need to supply your own install media for
a Windows install (thanks Microsoft):

```json
"iso_url": "Win10_1803_English_x64.iso",
"iso_checksum": "sha1:08fbb24627fa768f869c09f44c5d6c1e53a57a6f"
```

Next, add some scripts to provision the install:

```sh
# Example provisioner script
echo 'Set-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control\Power\ -name HibernateEnabled -value 0' >disable_hibernate.ps1
```

And add it to the goldboot config in the order they should be executed:
```json
"provisioners": [
	{
		"type": "shell",
		"script": "disable_hibernate.ps1"
	}
]
```

Now, build the image:
```sh
goldboot build
```

And finally deploy it to a physical disk:
```sh
# THIS WILL OVERWRITE /dev/sdX! TAKE A BACKUP FIRST!
goldboot image write WindowsMachine /dev/sdX
```