use std::error::Error;
use std::path::{PathBuf, Path};
use std::{fs, env, io};
use structopt::StructOpt;
use csv;
use cursive::Cursive;
use cursive::views::{Dialog, SelectView, EditView, ViewRef, ScrollView};
use cursive::traits::{Identifiable, Boxable};
use std::cell::{RefCell, RefMut};
use std::borrow::{Borrow, BorrowMut};
use std::rc::Rc;
use std::process::Command;


#[derive(Debug, StructOpt)]
#[structopt(name = "image-captioner", about = "Edit captions for a gallery of images.")]
struct Opt {
    /// Directory of the gallery to generate captions for
    #[structopt(parse(from_os_str))]
    gallery_dir: Option<PathBuf>,

    /// The type of output, available options: "csv"
    #[structopt(short = "t", long = "output-type", default_value = "csv")]
    output_type: String,

    /// The name of the output file (if there is one).
    /// Will be "captions.csv" by default for the "csv" output-type.
    #[structopt(short = "n", long = "output-name")]
    output_name: Option<String>,

    /// whether or not to edit the captions
    #[structopt(short = "e", long = "edit")]
    edit: bool,

    /// The command used to launch an image viewer
    /// upon editing the caption for an image in order
    /// to view the image who's caption is being edited
    #[structopt(short = "c", long = "view-command")]
    view_command: Option<String>,

    /// The command used to launch an image viewer
    /// upon editing the caption for an image in order
    /// to view the image who's caption is being edited.
    /// Escape dash "-" symbols with a backslash: "\-".
    /// For example: -a "\-\-some" "command"
    #[structopt(short = "a", long = "view-command-args")]
    view_command_args: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct CaptionRecord {
    /// Path to the image being captioned
    pub image_path: PathBuf,
    
    /// Caption for the image
    pub caption: String,
}

impl CaptionRecord {
    /// Create a new CaptionRecord
    fn new(image_path: &PathBuf, caption: String) -> CaptionRecord {
        CaptionRecord {
            image_path: image_path.clone(),
            caption: caption.clone(),
        }
    }

    /// Create a new empty CaptionRecord
    fn empty_caption(image_path: &PathBuf) -> CaptionRecord {
        CaptionRecord::new(image_path, String::new())
    }

    /// Get the name of the image file associated with this
    /// CaptionRecord.
    fn get_filename(&self) -> &str {
        self.image_path.file_name().unwrap().to_str().unwrap()
    }

    /// Get a label representing this CaptionRecord.
    fn get_label(&self) -> String {
        format!("{}: {}", self.get_filename(), self.caption)
    }
}

/// A command for previewing an image, to be executed
/// in the shell/command line.
#[derive(Debug, Clone)]
struct ViewCommand {
    /// The name/path to the executable to be executed
    pub command: String,

    /// The arguments to supply when running the command
    pub args: Option<Vec<String>>,
}

impl ViewCommand {
    /// Create a new ViewCommand
    pub fn new(command: &String, args: &Option<Vec<String>>) -> ViewCommand {
        ViewCommand {
            command: command.clone(),
            args: args.clone(),
        }
    }
}

/// Get a Vec of paths to image files in the specified gallery_dir
/// directory path. Or get an error if there was a problem.
fn get_image_files(gallery_dir: &PathBuf) -> io::Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let supported_extensions = vec!["jpg", "jpeg", "png"];

    for entry in fs::read_dir(gallery_dir)? {
        let entry_path = entry?.path().clone();

        match entry_path.extension() {
            Some(ext) => {
                let ext_string = ext.to_str().expect("unable to convert path").to_lowercase();
                let ext_str = ext_string.as_str();

                if supported_extensions.contains(&ext_str)
                {
                    paths.push(entry_path);
                }
            },
            None => ()
        }
    }

    Ok(paths)
}

/// Generate a Vec of empty CaptionRecord from a Vec of image paths
fn generate_empty_captions(image_paths: &Vec<PathBuf>) -> Vec<CaptionRecord> {
    let mut records: Vec<CaptionRecord> = Vec::new();

    for image_path in image_paths {
        records.push(CaptionRecord::empty_caption(&image_path))
    }

    return records;
}

/// Read a CSV file which specifies captions, and create a Vec of
/// CaptionRecord, or an Error if there was a problem doing this.
/// csv_path is the path to where the CSV file is located.
/// 
/// ```csv
/// Image Path,Caption
/// example.jpg,This is an example caption
/// example2.jpg,Another example
/// ```
fn read_caption_csv(csv_path: &Path) -> Result<Vec<CaptionRecord>, Box<dyn Error>> {
    let image_directory = csv_path.parent().expect("csv path is not a valid file").to_path_buf();
    let mut captions: Vec<CaptionRecord> = Vec::new();
    let mut rdr = csv::Reader::from_path(csv_path)?;

    for item in rdr.records() {
        let record = item?;
        let image_filename = record.get(0).expect("badly formatted image filename in csv");
        let caption = record.get(1).expect("badly formatted caption entry in csv");

        let image_path = image_directory.join(image_filename);

        let caption_record = CaptionRecord::new(&image_path, caption.to_owned());

        captions.push(caption_record);
    }

    return Ok(captions);
}

/// Write a Vec of CaptionRecord to a CSV file with the specified
/// csv_path. 
fn write_caption_csv(records: &Vec<CaptionRecord>, csv_path: &Path) -> Result<(), Box<dyn Error>> {
    println!("Writing captions to \"{}\".", csv_path.display());

    let mut wtr = csv::Writer::from_path(csv_path)?;
    wtr.write_record(&["Image", "Caption"])?;

    for record in records {
        let image_filename: &str = record.image_path.file_name().expect("expected image to be a filename").to_str().unwrap();
        wtr.write_record(&[image_filename, record.caption.as_str()])?;
    }

    Ok(())
}

/// Callback to be used when the Ok button is pressed in the 
/// edit caption dialog.
fn submit_callback(s: &mut Cursive) {
    let new_caption_text: Rc<String> = s.call_on_id("edit_caption", |view: &mut EditView| {
    view.get_content()
    }).unwrap().clone();

    let mut select_view_ref: ViewRef<SelectView<Rc<RefCell<CaptionRecord>>>> = s.find_id::<SelectView<Rc<RefCell<CaptionRecord>>>>("select_image").unwrap();

    let selection = Rc::clone(select_view_ref.selection().unwrap().as_ref());
    let mut record_ref: RefMut<CaptionRecord> = RefCell::borrow_mut(Rc::borrow(&selection));
    record_ref.caption = new_caption_text.as_ref().clone();

    let selected_id = select_view_ref.selected_id().unwrap();
    let select_view_ref_mut = select_view_ref.borrow_mut();

    //remove and insert again to get around limitation of Cursive UI not refreshing list
    select_view_ref_mut.remove_item(selected_id);
    select_view_ref_mut.insert_item(selected_id, record_ref.get_label(), selection.clone());
    select_view_ref_mut.set_selection(selected_id);

    s.pop_layer();
}

/// Function triggered when the user wants to edit the caption
/// for a selected image. Runs the ViewCommand (if specified by the user),
/// and shows the edit caption dialog.
fn edit_caption(view_command: &Option<ViewCommand>, s: &mut Cursive, record: Rc<RefCell<CaptionRecord>>) {
    let record_ref = RefCell::borrow(record.borrow());
    let caption_text = record_ref.caption.clone();
    let image_file_name = String::from(record_ref.get_filename().clone());

    match view_command {
        Some(command) => {
            let image_path = record_ref.image_path.to_str().unwrap();

            let args = command.args.clone();

            let mut c = Command::new(command.command.clone());

            match args {
                Some(_args) => {
                    for arg in _args {
                        c.arg(arg.replace("\\", ""));
                    }
                }
                None => ()
            }

            c.arg(image_path);
            c.output().expect("unable to launch image editor");
        }
        None => ()
    }


    let mut ev = EditView::new();
    ev.set_content(caption_text);
    ev.set_on_submit(|s, _| {
        submit_callback(s);
    });

    s.add_layer(Dialog::around(ev.with_id("edit_caption")
            .fixed_width(10))
        .title(format!("Editing caption for image {}", image_file_name))
        .button("Ok", submit_callback)
        .button("Cancel", |s| {
            s.pop_layer();
        }));
}

/// Shows a command line GUI using the cursive library, for editing
/// the captions.
fn edit_captions(opt: &Opt, captions: &Vec<CaptionRecord>) -> Vec<CaptionRecord> {
    if opt.edit == false {
        return captions.clone();
    }

    let mut editable_captions: Vec<Rc<RefCell<CaptionRecord>>> = Vec::new();

    for record in captions {
        editable_captions.push(Rc::new(RefCell::new(record.clone())))
    }

    // Creates the cursive root - required for every application.
    let mut siv = Cursive::default();

    let mut select_view = SelectView::<Rc<RefCell<CaptionRecord>>>::new();

    for record in &editable_captions {
        let record_reference = RefCell::borrow(record.borrow());
        let label = record_reference.get_label();
        select_view.add_item(label, record.clone());
    }

    let view_command: Option<ViewCommand> = match &opt.view_command {
        Some(command) => Some(ViewCommand::new(command, &opt.view_command_args)),
        None => None
    };

    let view_command_rc = Rc::new(view_command);

    select_view.set_on_submit(move |s, record: &Rc<RefCell<CaptionRecord>>| {
        let vc = view_command_rc.clone();
        edit_caption(vc.as_ref(), s, record.clone());
    });

    // Creates a dialog with a single "Ok" button
    siv.add_layer(
        Dialog::around(
            ScrollView::new(
                select_view.with_id("select_image")
            )
        )
            .title("Caption Editor")
            .button("Ok", |s| s.quit())
    );

    // Starts the event loop.
    siv.run();

    let mut new_captions: Vec<CaptionRecord> = Vec::new();
    for record_ref in editable_captions {
        let record_ref_rc = record_ref.clone();
        let new_record: CaptionRecord = RefCell::borrow(record_ref_rc.borrow()).clone();
        new_captions.push(new_record);
    }

    return new_captions;
}

fn main() {
    let opt = Opt::from_args();

    println!("{:?}", opt);

    let gallery_dir = match opt.gallery_dir.clone() {
        Some(path) => path,
        None => env::current_dir().expect("Error: cannot get current directory")
    };

    let image_paths = get_image_files(&gallery_dir).expect("Error: unable to read image files from gallery directory");

    let output_type: String = opt.output_type.clone();
    match output_type.as_str() {
       "csv" => {
           let csv_filename: String = match opt.output_name.clone() {
               Some(name) => name,
               None => String::from("captions.csv")
           };

           let csv_path = gallery_dir.join(Path::new(csv_filename.as_str()));

           let mut captions = if csv_path.exists()
           {
               println!("Caption file \"{}\" already exists, reading file.", csv_filename);
               let mut captions = read_caption_csv(csv_path.as_path()).expect("unable to read captions csv");
               let mut images_with_no_cations: Vec<PathBuf> = Vec::new();

               for image_path in image_paths {
                   match captions.iter().find(|&record| {
                       record.image_path.canonicalize().unwrap().eq(&image_path.canonicalize().unwrap())
                   }) {
                       Some(_record) => (),
                       None => images_with_no_cations.push(image_path)
                   }
               }

               let mut new_captions = generate_empty_captions(&images_with_no_cations);

               println!("Appending the following new images: [{}]", new_captions.iter()
                   .fold(String::new(), |acc, record| {
                       acc + &record.image_path.file_name().unwrap().to_str().unwrap() + ", "
                   }));

               captions.append(&mut new_captions);

               captions.sort_by(|a, b| {
                   a.image_path.file_name().unwrap().cmp(b.image_path.file_name().unwrap())
               });

               captions
           } else {
               println!("Generating new captions.");
               generate_empty_captions(&image_paths)
           };

           captions = edit_captions(&opt, &mut captions);

           write_caption_csv(&captions, csv_path.as_path()).expect("unable to write captions to csv");
       },
        _ => println!("Error: unsupported output type {}", output_type)
    }
}


